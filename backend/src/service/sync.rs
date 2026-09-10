/// git_sync: 将 pod 内指定路径 push 到 git 仓库的 agent-{id} 分支
///
/// 方案：通过 K8s exec 在 pod 内执行 shell 命令完成 git 操作。
/// 要求 pod 镜像内已安装 git；ssh 认证时需在 pod 内预置 known_hosts 或通过 GIT_SSH_COMMAND 跳过主机校验。
use anyhow::{anyhow, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entity::{agent, organization};
use crate::service::k8s::{client_from_kubeconfig, exec_in_pod};

pub struct GitSyncInput {
    pub org_id: Uuid,
    pub paths: Vec<String>,
    pub commit_message: Option<String>,
}

/// 并发同步组织下所有 Running 状态的 agent pod
pub async fn git_sync_org_agents(db: &DatabaseConnection, input: GitSyncInput) -> Result<()> {
    // 获取组织 kubeconfig
    let org = organization::Entity::find_by_id(input.org_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("组织不存在"))?;

    let k8s_client = client_from_kubeconfig(org.k8s_kubeconfig.as_deref()).await?;

    // 合并 git 配置：请求参数优先，fallback 到 org 配置
    let git_url = org
        .git_url
        .ok_or_else(|| anyhow!("未配置 git_url，请在组织设置或请求中提供"))?;
    let git_username = org.git_username;
    let git_password = org.git_password;
    let ssh_private_key = org.git_ssh_private_key;

    // 查找该组织下所有有 pod_name 的 agent
    let agents = agent::Entity::find()
        .filter(agent::Column::OrgId.eq(input.org_id))
        .filter(agent::Column::PodName.is_not_null())
        .all(db)
        .await?;

    if agents.is_empty() {
        tracing::info!("org {} 下无可同步的 agent", input.org_id);
        return Ok(());
    }

    tracing::info!("开始同步 org {} 下 {} 个 agent", input.org_id, agents.len());

    // 并发执行，每个 agent 独立分支
    let mut handles = Vec::new();
    for agent_rec in agents {
        let client = k8s_client.clone();
        let git_url = git_url.clone();
        let git_username = git_username.clone();
        let git_password = git_password.clone();
        let ssh_private_key = ssh_private_key.clone();
        let paths = input.paths.clone();
        let commit_message = input
            .commit_message
            .clone()
            .unwrap_or_else(|| "chore: sync agent state".to_string());
        let agent_id = agent_rec.id;
        let pod_name = match agent_rec.pod_name.clone() {
            Some(n) => n,
            None => continue,
        };
        let namespace = agent_rec
            .pod_namespace
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let branch = format!("agent-{}", agent_id);

        handles.push(tokio::spawn(async move {
            let result = sync_one_pod(
                &client,
                &pod_name,
                &namespace,
                &branch,
                &git_url,
                git_username.as_deref(),
                git_password.as_deref(),
                ssh_private_key.as_deref(),
                &paths,
                &commit_message,
            )
            .await;

            match &result {
                Ok(_) => tracing::info!("agent {} git sync 成功", agent_id),
                Err(e) => tracing::error!("agent {} git sync 失败: {}", agent_id, e),
            }
            result
        }));
    }

    // 等待所有任务完成，收集错误
    let mut errors = Vec::new();
    for h in handles {
        if let Ok(Err(e)) = h.await {
            errors.push(e.to_string());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("部分 agent 同步失败:\n{}", errors.join("\n")))
    }
}

/// 在单个 pod 内通过 kubectl exec 执行 git 同步脚本
async fn sync_one_pod(
    client: &kube::Client,
    pod_name: &str,
    namespace: &str,
    branch: &str,
    git_url: &str,
    git_username: Option<&str>,
    git_password: Option<&str>,
    ssh_private_key: Option<&str>,
    paths: &[String],
    commit_message: &str,
) -> Result<()> {
    // Deployment 的 Pod 名不固定，按 label 查找当前 Pod
    let pod_full_name = crate::service::k8s::find_agent_pod(client, namespace, pod_name)
        .await?
        .and_then(|p| p.metadata.name)
        .ok_or_else(|| anyhow!("未找到 Pod (app={})", pod_name))?;

    // 认证方式：优先 SSH key，其次 https username/password，最后裸 url
    let effective_url = build_git_url(git_url, git_username, git_password);

    // 构造在 pod 内执行的 shell 脚本（单行，分号分隔）
    let script = build_sync_script(
        &effective_url,
        branch,
        paths,
        commit_message,
        ssh_private_key,
    );

    exec_in_pod(client, &pod_full_name, namespace, &script).await
}

/// 构造 git 操作脚本（单行 shell，适合 exec cmd 传入）
fn build_sync_script(
    git_url: &str,
    branch: &str,
    paths: &[String],
    commit_message: &str,
    ssh_private_key: Option<&str>,
) -> String {
    // 工作目录：在 pod /tmp 下使用一个唯一临时目录，避免并发冲突
    let work_dir = format!("/tmp/git-sync-{}", branch.replace('/', "-"));

    // 路径列表，空格分隔（cp 用）
    let path_list = paths.join(" ");

    // SSH key 处理：写入临时文件，通过 GIT_SSH_COMMAND 引用
    let ssh_setup = match ssh_private_key {
        Some(key) => {
            // 用 printf 写入避免多行 echo 问题；key 中的特殊字符用 base64 传递
            let key_b64 = base64_encode(key);
            format!(
                "KEY_FILE=$(mktemp); printf '%s' '{}' | base64 -d > \"$KEY_FILE\"; chmod 600 \"$KEY_FILE\"; export GIT_SSH_COMMAND=\"ssh -i $KEY_FILE -o StrictHostKeyChecking=no\";",
                key_b64
            )
        }
        None => String::new(),
    };

    let ssh_cleanup = if ssh_private_key.is_some() {
        "rm -f \"$KEY_FILE\";"
    } else {
        ""
    };

    // 脚本逻辑：
    // 1. 清理旧目录，初始化或 clone
    // 2. checkout 目标分支（不存在则从孤立分支创建）
    // 3. 从各路径复制文件
    // 4. git add / commit / push
    format!(
        "{ssh_setup} \
        rm -rf {work_dir} && mkdir -p {work_dir} && cd {work_dir} && \
        git init && \
        git remote add origin '{git_url}' && \
        export GIT_TERMINAL_PROMPT=0 && \
        git fetch origin {branch} 2>/dev/null && git checkout {branch} 2>/dev/null || git checkout --orphan {branch} && \
        for SRC in {path_list}; do \
          if [ -d \"$SRC\" ]; then \
            mkdir -p \"$(basename $SRC)\" && cp -r \"$SRC/.\" \"$(basename $SRC)/\"; \
          elif [ -f \"$SRC\" ]; then \
            mkdir -p \"$(dirname $(basename $SRC))\" && cp \"$SRC\" \"$(basename $SRC)\"; \
          fi; \
        done && \
        git config user.email 'agent-sync@vibeyeah' && \
        git config user.name 'Agent Sync' && \
        git add -A && \
        git diff --cached --quiet || git commit -m '{commit_message}' && \
        git push origin {branch} && \
        cd / && rm -rf {work_dir}; \
        {ssh_cleanup}",
        ssh_setup = ssh_setup,
        work_dir = work_dir,
        git_url = git_url,
        branch = branch,
        path_list = path_list,
        commit_message = commit_message,
        ssh_cleanup = ssh_cleanup,
    )
}

/// 将 username:password 嵌入 https url（如果提供）
fn build_git_url(url: &str, username: Option<&str>, password: Option<&str>) -> String {
    match (username, password) {
        (Some(u), Some(p)) if url.starts_with("https://") => {
            // https://user:pass@host/path
            let rest = &url["https://".len()..];
            // URL encode 用户名和密码中的特殊字符
            let u_enc = url_encode(u);
            let p_enc = url_encode(p);
            format!("https://{}:{}@{}", u_enc, p_enc, rest)
        }
        _ => url.to_string(),
    }
}

/// 简单的 URL 百分比编码（仅编码 @ : 等对 URL 有歧义的字符）
fn url_encode(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '@' => vec!['%', '4', '0'],
            ':' => vec!['%', '3', 'A'],
            '#' => vec!['%', '2', '3'],
            '?' => vec!['%', '3', 'F'],
            ' ' => vec!['%', '2', '0'],
            _ => vec![c],
        })
        .collect()
}

fn base64_encode(s: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD.encode(s.as_bytes())
}

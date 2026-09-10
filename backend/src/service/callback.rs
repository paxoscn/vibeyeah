/// 外部 agent 调用路由（/callback/{skill}/{user_id}）
///
/// 流程：
/// 1. 扫描 NAS `{nas_root}/vibeyeah/agents/{agent}/users/{user_id}/home/.hermes/skills/**/{skill}`，
///    找到安装了该技能的若干 agent 目录。
/// 2. 命中后记录日志并立即向上游返回成功。
/// 3. 后台依次对候选 agent 通过 K8s exec 在其 pod 内以 oneshot 模式运行
///    `hermes chat -s {skill}`（HERMES_HOME 指向该用户的 NAS home，即“以用户身份”），
///    携带技能名、query 参数与消息体；某个成功即结束遍历，失败则记录日志后继续尝试下一个。
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entity::{agent, organization};
use crate::service::k8s::client_from_kubeconfig;

/// 一次回调请求的上下文
#[derive(Debug, Clone)]
pub struct CallbackInput {
    pub skill: String,
    pub user_id: String,
    /// 原始 query 参数（保留出现顺序，允许重复键）
    pub query: Vec<(String, String)>,
    /// 请求体（如无则为 None）
    pub body: Option<String>,
}

/// 递归判断 `root` 下（任意深度）是否存在名为 `skill` 的文件或目录。
/// `skills/**/{skill}` 中的 `**` 即任意层级。
fn contains_skill(root: &Path, skill: &str, max_depth: usize) -> bool {
    if max_depth == 0 {
        return false;
    }
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy() == skill {
            return true;
        }
        let path = entry.path();
        if path.is_dir() && contains_skill(&path, skill, max_depth - 1) {
            return true;
        }
    }
    false
}

/// 扫描 NAS，返回所有为该 `user_id` 安装了 `skill` 的 agent 目录名。
///
/// 路径模板：`{nas_root}/vibeyeah/agents/{agent}/users/{user_id}/home/.hermes/skills/**/{skill}`
pub fn find_agents_with_skill(nas_root: &str, user_id: &str, skill: &str) -> Vec<String> {
    let agents_dir = Path::new(nas_root).join("vibeyeah").join("agents");
    let mut matched = Vec::new();

    let entries = match std::fs::read_dir(&agents_dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                "[callback] 无法读取 agents 目录 {}: {}",
                agents_dir.display(),
                e
            );
            return matched;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let agent_dir = entry.file_name().to_string_lossy().to_string();
        let skills_dir = path
            .join("users")
            .join(user_id)
            .join("home")
            .join(".hermes")
            .join("skills");
        if contains_skill(&skills_dir, skill, 8) {
            matched.push(agent_dir);
        }
    }

    matched.sort();
    matched
}

/// 将 NAS 上的 agent 目录名解析为数据库中的 agent 记录。
/// 目录名既可能是 Deployment 名（`pod_name`），也可能是 agent 的 UUID。
async fn resolve_agent(db: &DatabaseConnection, dir_name: &str) -> Option<agent::Model> {
    if let Ok(Some(a)) = agent::Entity::find()
        .filter(agent::Column::PodName.eq(dir_name))
        .one(db)
        .await
    {
        return Some(a);
    }
    if let Ok(uuid) = Uuid::parse_str(dir_name) {
        if let Ok(Some(a)) = agent::Entity::find_by_id(uuid).one(db).await {
            return Some(a);
        }
    }
    None
}

/// 构造发给 hermes 的提示词：技能名 + query 参数 + 消息体
fn build_prompt(input: &CallbackInput) -> String {
    let mut prompt = format!("请使用技能「{}」处理以下外部回调请求。", input.skill);

    if !input.query.is_empty() {
        let map: serde_json::Map<String, serde_json::Value> = input
            .query
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        prompt.push_str(&format!("\n查询参数: {}", serde_json::Value::Object(map)));
    }

    if let Some(body) = &input.body {
        if !body.trim().is_empty() {
            prompt.push_str(&format!("\n消息体: {}", body));
        }
    }

    prompt
}

/// 通过 K8s exec 在单个 agent pod 内以 oneshot 模式调用 hermes CLI。
///
/// 以 `HERMES_HOME` 指向该用户在 NAS 上的 hermes home
/// （`/data/nas/vibeyeah/agents/{agent_dir}/users/{user_id}/home/.hermes`），
/// 使本次运行加载该用户自己的配置 / 技能 / 会话，即“以用户身份”调用。
/// 命令：`hermes chat --oneshot -s {skill} --query-file <prompt>`，退出码 0 视为成功。
async fn invoke_hermes(
    db: &DatabaseConnection,
    agent_rec: &agent::Model,
    agent_dir: &str,
    exec_timeout_secs: u64,
    input: &CallbackInput,
) -> Result<()> {
    let pod_name = agent_rec
        .pod_name
        .as_deref()
        .ok_or_else(|| anyhow!("agent {} 尚无 pod_name", agent_rec.id))?;
    let namespace = agent_rec
        .pod_namespace
        .clone()
        .unwrap_or_else(|| "default".into());

    // 按 agent 所属组织构建 kube client（支持组织专属集群；exec 走 K8s API，跨集群同样可达）
    let kubeconfig_b64 = if let Some(org_id) = agent_rec.org_id {
        organization::Entity::find_by_id(org_id)
            .one(db)
            .await
            .ok()
            .flatten()
            .and_then(|o| o.k8s_kubeconfig)
    } else {
        None
    };
    let client = client_from_kubeconfig(kubeconfig_b64.as_deref()).await?;

    // Deployment 的 Pod 名不固定，按 label 查找当前 Pod
    let pod_full = crate::service::k8s::find_agent_pod(&client, &namespace, pod_name)
        .await?
        .and_then(|p| p.metadata.name)
        .ok_or_else(|| anyhow!("未找到 Pod (app={})", pod_name))?;
    let script = build_exec_script(agent_dir, exec_timeout_secs, input);

    crate::service::k8s::exec_in_pod(&client, &pod_full, &namespace, &script)
        .await
        .with_context(|| {
            format!(
                "pod {} 内调用 hermes 失败 (agent={} skill={} user={})",
                pod_full, agent_rec.id, input.skill, input.user_id
            )
        })?;

    tracing::info!(
        "[callback] hermes 调用成功 agent={} user={} skill={}",
        agent_rec.id,
        input.user_id,
        input.skill
    );
    Ok(())
}

/// NAS 在 agent pod 内的挂载点（与 service::k8s::NAS_MOUNT_PATH 保持一致）
const POD_NAS_MOUNT: &str = "/data/nas";

/// 由 user_id 派生合法的 Linux 用户名（须与 entrypoint.sh 的派生一致）：
/// 前缀 u + 小写、仅保留字母数字、截断到 32 位。
fn linux_user_from_id(user_id: &str) -> String {
    format!("u{}", user_id)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(32)
        .collect()
}

/// 构造在 pod 内执行的 hermes 调用脚本。
/// skill / user_id / agent_dir 已在 handler 中限定为 [A-Za-z0-9._-]，
/// prompt 经 base64 传输，均无 shell 注入风险。
///
/// 容器以 uid 1000(agent) 运行、且 agent 具有免密 sudo；而用户的 .hermes 归属其
/// 独立 Linux 账户（属主非 agent、权限 600），因此必须以该用户身份运行 hermes
/// （与 entrypoint.sh 启动 gateway 的方式一致），否则读取 .env 会 Permission denied。
/// 内层脚本用单引号包裹且内部只用双引号，外层 shell 会原样传给 `bash -c`。
fn build_exec_script(agent_dir: &str, exec_timeout_secs: u64, input: &CallbackInput) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    let user_home = format!(
        "{POD_NAS_MOUNT}/vibeyeah/agents/{agent_dir}/users/{user}/home",
        user = input.user_id
    );
    let prompt_b64 = STANDARD.encode(build_prompt(input).as_bytes());
    let linux_user = linux_user_from_id(&input.user_id);

    format!(
        r#"sudo -n -u '{linux_user}' bash -c '
export HOME="{user_home}"
export HERMES_HOME="{user_home}/.hermes"
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
if [ ! -d "$HERMES_HOME" ]; then
    echo "[callback] hermes home 不存在: $HERMES_HOME" >&2
    exit 1
fi
cd "{user_home}" || exit 1
PROMPT="$(printf "%s" "{prompt_b64}" | base64 -d)"
timeout {secs} /usr/local/bin/hermes chat -q "$PROMPT" --quiet -s "{skill}"
RC=$?
exit $RC
'"#,
        linux_user = linux_user,
        user_home = user_home,
        prompt_b64 = prompt_b64,
        secs = exec_timeout_secs,
        skill = input.skill,
    )
}

/// 后台派发：依次尝试各候选 agent，成功即返回，失败记录日志后继续。
pub async fn dispatch(
    db: &DatabaseConnection,
    exec_timeout_secs: u64,
    agent_dirs: Vec<String>,
    input: CallbackInput,
) {
    for dir in agent_dirs {
        let agent_rec = match resolve_agent(db, &dir).await {
            Some(a) => a,
            None => {
                tracing::warn!(
                    "[callback] 无法将 agent 目录 {} 解析为 agent 记录，跳过",
                    dir
                );
                continue;
            }
        };

        match invoke_hermes(db, &agent_rec, &dir, exec_timeout_secs, &input).await {
            Ok(()) => {
                tracing::info!(
                    "[callback] 调用 hermes 成功，结束遍历 agent={} skill={} user={}",
                    agent_rec.id,
                    input.skill,
                    input.user_id
                );
                return;
            }
            Err(e) => {
                tracing::error!(
                    "[callback] 调用 hermes 失败 agent={} dir={} skill={} user={}: {}",
                    agent_rec.id,
                    dir,
                    input.skill,
                    input.user_id,
                    e
                );
                // 失败则继续遍历下一个 agent
            }
        }
    }

    tracing::error!(
        "[callback] 所有候选 agent 均调用失败 skill={} user={}",
        input.skill,
        input.user_id
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 构造 {root}/vibeyeah/agents/{agent}/users/{user}/home/.hermes/skills/{rel}
    fn touch(root: &Path, agent: &str, user: &str, rel: &str) {
        let p = root
            .join("vibeyeah")
            .join("agents")
            .join(agent)
            .join("users")
            .join(user)
            .join("home")
            .join(".hermes")
            .join("skills")
            .join(rel);
        fs::create_dir_all(&p).unwrap();
        // 技能以目录形式存在（内含 SKILL.md）
        fs::write(p.join("SKILL.md"), "# test skill").unwrap();
    }

    #[test]
    fn test_find_agents_with_skill() {
        let tmp = std::env::temp_dir().join(format!("cb-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();

        // agent-a：技能在一级目录下
        touch(&tmp, "agent-a", "u1", "myskill");
        // agent-b：技能在多层嵌套目录下（** 任意深度）
        touch(&tmp, "agent-b", "u1", "category/sub/myskill");
        // agent-c：u1 没有该技能（给另一个用户装了）
        touch(&tmp, "agent-c", "u2", "myskill");
        // agent-d：u1 装了别的技能
        touch(&tmp, "agent-d", "u1", "otherskill");

        let mut matched = find_agents_with_skill(tmp.to_str().unwrap(), "u1", "myskill");
        matched.sort();
        assert_eq!(matched, vec!["agent-a".to_string(), "agent-b".to_string()]);

        // 未命中
        let none = find_agents_with_skill(tmp.to_str().unwrap(), "u1", "nosuchskill");
        assert!(none.is_empty());

        // 目录不存在也不应 panic
        let empty = find_agents_with_skill("/nonexistent/path", "u1", "myskill");
        assert!(empty.is_empty());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_build_exec_script() {
        use base64::Engine as _;
        let input = CallbackInput {
            skill: "my-skill".to_string(),
            user_id: "u1".to_string(),
            query: vec![("foobar".to_string(), "1".to_string())],
            body: Some("hello \"world\" $(rm -rf)".to_string()),
        };
        let script = build_exec_script("agent-abc", 900, &input);

        // 以该用户的 NAS home 作为 HERMES_HOME（“以用户身份”）
        assert!(script
            .contains("HERMES_HOME=\"/data/nas/vibeyeah/agents/agent-abc/users/u1/home/.hermes\""));
        // 以该用户的 Linux 账户运行 hermes（与 entrypoint 一致，避免读取 .env 时 Permission denied）
        assert!(script.contains("sudo -n -u 'uu1' bash -c"));
        // 单查询(-q) + 指定技能；prompt 经 base64 解码为变量后作为单一参数传入
        assert!(script.contains("hermes chat -q \"$PROMPT\" --quiet -s \"my-skill\""));
        assert!(script.contains("timeout 900"));
        // prompt 以 base64 传输（原文中的引号 / $() 不会进入 shell）
        assert!(!script.contains("$(rm -rf)"));
        let b64 = base64::engine::general_purpose::STANDARD.encode(build_prompt(&input).as_bytes());
        assert!(script.contains(&b64));
    }
}

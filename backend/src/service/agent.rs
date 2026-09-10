use anyhow::{anyhow, Result};
use chrono::Utc;
use kube::Client;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use uuid::Uuid;

use crate::config::Config;
use crate::entity::{
    agent::{self, PodStatus},
    user_agent,
};
use crate::service::k8s;

pub struct CreateAgentInput {
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub user_id: Uuid,
    /// 所属组织，必填
    pub org_id: Uuid,
    // 飞书 Bot 配置（注入到容器）
    pub lark_app_id: Option<String>,
    pub lark_app_secret: Option<String>,
    pub lark_bot_open_id: Option<String>,
    pub lark_bot_name: Option<String>,
    // 微信配置
    pub wechat_account_id: Option<String>,
    pub wechat_token: Option<String>,
    pub wechat_base_url: Option<String>,
    pub wechat_user_id: Option<String>,
}

/// 创建 agent：校验成员资格 → 读取组织 k8s_namespace → 建记录 → 启动 Deployment
pub async fn create_agent(
    db: &DatabaseConnection,
    _default_k8s: &Client,
    config: &Config,
    input: CreateAgentInput,
) -> Result<agent::Model> {
    use crate::entity::organization;
    use crate::service::organization as org_service;

    // 验证用户是该组织成员
    org_service::get_member(db, &input.org_id, &input.user_id)
        .await?
        .ok_or_else(|| anyhow!("您不是该组织的成员"))?;

    // 读取组织配置（namespace + 专属 kubeconfig）
    let org = organization::Entity::find_by_id(input.org_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("组织不存在"))?;

    let namespace = org.k8s_namespace.clone();

    // 优先使用组织专属 kubeconfig，否则回退全局默认
    let k8s_client = k8s::client_from_kubeconfig(org.k8s_kubeconfig.as_deref())
        .await
        .map_err(|e| anyhow!("构建组织 K8s client 失败: {e}"))?;
    let now = Utc::now();
    let agent_id = Uuid::new_v4();

    let new_agent = agent::ActiveModel {
        id: Set(agent_id),
        name: Set(input.name.clone()),
        description: Set(input.description.clone()),
        system_prompt: Set(input.system_prompt.clone()),
        created_by: Set(Some(input.user_id)),
        org_id: Set(Some(input.org_id)),
        pod_status: Set(PodStatus::Creating),
        pod_name: Set(None),
        pod_namespace: Set(Some(namespace.clone())),
        pod_synced_at: Set(None),
        webrtc_url: Set(None),
        stream_active: Set(false),
        lark_app_id: Set(input.lark_app_id.clone()),
        lark_app_secret: Set(input.lark_app_secret.clone()),
        lark_bot_open_id: Set(input.lark_bot_open_id.clone()),
        lark_bot_name: Set(input.lark_bot_name.clone()),
        wechat_account_id: Set(input.wechat_account_id.clone()),
        wechat_token: Set(input.wechat_token.clone()),
        wechat_base_url: Set(input.wechat_base_url.clone()),
        wechat_user_id: Set(input.wechat_user_id.clone()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let agent_rec = new_agent.insert(db).await?;

    // 建立用户分配关系
    let ua = user_agent::ActiveModel {
        user_id: Set(input.user_id),
        agent_id: Set(agent_id),
        assigned_at: Set(now.into()),
        ..Default::default()
    };
    ua.insert(db).await?;

    // 启动 K8s Deployment（使用组织专属 client）
    let lark_env = match (
        agent_rec.lark_app_id.clone(),
        agent_rec.lark_app_secret.clone(),
        agent_rec.lark_bot_open_id.clone(),
    ) {
        (Some(app_id), Some(app_secret), Some(bot_open_id))
            if !app_id.is_empty() && !app_secret.is_empty() =>
        {
            Some(k8s::LarkEnv {
                app_id,
                app_secret,
                bot_open_id,
            })
        }
        _ => None,
    };

    let wechat_env = match (
        agent_rec.wechat_account_id.clone(),
        agent_rec.wechat_token.clone(),
        agent_rec.wechat_base_url.clone(),
        agent_rec.wechat_user_id.clone(),
    ) {
        (Some(aid), Some(tok), Some(base), Some(uid)) if !aid.is_empty() && !tok.is_empty() => {
            Some(k8s::WechatEnv {
                account_id: aid,
                token: tok,
                base_url: base,
                user_id: uid,
            })
        }
        _ => None,
    };

    // 创建 Pod 之前先准备该用户的 NAS home（及 agent 级 Claude Code 配置），
    // 保证 Pod 启动后其 gateway / Claude Code 即可运行；失败则不创建 K8s 资源。
    // 拷贝完成后会把组织的 OpenAI 配置（base_url/api_key/model）渲染进
    // `.hermes/.env`、`.hermes/config.yaml` 与 `.claude/settings.json`。
    let agent_dir = k8s::agent_workload_name(&agent_id);
    let prep_nas_root = config.nas_mount_root.clone();
    let prep_user_id = input.user_id.to_string();
    let prep_lark_id = input.lark_app_id.clone();
    let prep_lark_secret = input.lark_app_secret.clone();
    let openai_cfg = crate::service::user_home::OpenAiConfig {
        base_url: org.openai_base_url.clone().unwrap_or_default(),
        api_key: org.openai_api_key.clone().unwrap_or_default(),
        model: org.openai_model.clone().unwrap_or_default(),
    };
    let prep_result = match tokio::task::spawn_blocking(move || {
        crate::service::user_home::prepare_user_home(
            &prep_nas_root,
            &agent_dir,
            &prep_user_id,
            prep_lark_id.as_deref(),
            prep_lark_secret.as_deref(),
            &openai_cfg,
        )?;
        crate::service::user_home::prepare_agent_claude_settings(
            &prep_nas_root,
            &agent_dir,
            &openai_cfg,
        )
    })
    .await
    {
        Ok(inner) => inner,
        Err(join_err) => Err(anyhow!("准备用户 home 任务异常: {}", join_err)),
    };
    if let Err(e) = prep_result {
        tracing::error!("准备用户 home 失败 agent={}: {}", agent_id, e);
        let mut active: agent::ActiveModel = agent_rec.into();
        active.pod_status = Set(PodStatus::Failed);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        return Err(e);
    }

    match k8s::create_agent_deployment(
        &k8s_client,
        &agent_id,
        &namespace,
        org.image_pull_secret.clone(),
        &config.desktop_image,
        &config.sidecar_image,
        &config.nas_pvc_name,
        lark_env,
        wechat_env,
    )
    .await
    {
        Ok((pod_name, ns)) => {
            let webrtc_url = build_webrtc_url(&config.webrtc_base_url, &pod_name, &ns);
            let mut active: agent::ActiveModel = agent_rec.clone().into();
            active.pod_name = Set(Some(pod_name));
            active.pod_namespace = Set(Some(ns));
            active.pod_status = Set(PodStatus::Creating);
            active.webrtc_url = Set(Some(webrtc_url));
            active.updated_at = Set(Utc::now().into());
            Ok(active.update(db).await?)
        }
        Err(e) => {
            tracing::error!("K8s Deployment 创建失败 agent={}: {}", agent_id, e);
            let mut active: agent::ActiveModel = agent_rec.into();
            active.pod_status = Set(PodStatus::Failed);
            active.updated_at = Set(Utc::now().into());
            active.update(db).await?;
            Err(e)
        }
    }
}

/// 列出某组织下的所有 agent（调用方已验证成员资格）
pub async fn list_org_agents(db: &DatabaseConnection, org_id: &Uuid) -> Result<Vec<agent::Model>> {
    Ok(agent::Entity::find()
        .filter(agent::Column::OrgId.eq(*org_id))
        .all(db)
        .await?)
}

/// 开始推流
pub async fn start_stream(db: &DatabaseConnection, agent_id: &Uuid) -> Result<agent::Model> {
    let agent_rec = find_agent(db, agent_id).await?;
    let pod_name = agent_rec
        .pod_name
        .as_deref()
        .ok_or_else(|| anyhow!("Pod 尚未就绪"))?;
    let ns = agent_rec.pod_namespace.as_deref().unwrap_or("default");
    let k8s_client = org_k8s_client(db, &agent_rec).await?;

    let resp = k8s::call_stream_ctrl(&k8s_client, pod_name, ns, "start").await?;
    tracing::info!("stream-ctrl start: {:?}", resp);

    let now = Utc::now();
    let mut active: agent::ActiveModel = agent_rec.into();
    active.stream_active = Set(true);
    active.updated_at = Set(now.into());
    Ok(active.update(db).await?)
}

/// 停止推流
pub async fn stop_stream(db: &DatabaseConnection, agent_id: &Uuid) -> Result<agent::Model> {
    let agent_rec = find_agent(db, agent_id).await?;
    let pod_name = agent_rec
        .pod_name
        .as_deref()
        .ok_or_else(|| anyhow!("Pod 尚未就绪"))?;
    let ns = agent_rec.pod_namespace.as_deref().unwrap_or("default");
    let k8s_client = org_k8s_client(db, &agent_rec).await?;

    let resp = k8s::call_stream_ctrl(&k8s_client, pod_name, ns, "stop").await?;
    tracing::info!("stream-ctrl stop: {:?}", resp);

    let now = Utc::now();
    let mut active: agent::ActiveModel = agent_rec.into();
    active.stream_active = Set(false);
    active.updated_at = Set(now.into());
    Ok(active.update(db).await?)
}

// ── 内部 ──────────────────────────────────────────────────────────────────────

async fn find_agent(db: &DatabaseConnection, agent_id: &Uuid) -> Result<agent::Model> {
    agent::Entity::find_by_id(*agent_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("Agent 不存在"))
}

/// 根据 agent 所属组织的 kubeconfig 构建 K8s Client
async fn org_k8s_client(db: &DatabaseConnection, a: &agent::Model) -> Result<Client> {
    use crate::entity::organization;
    let kubeconfig_b64 = if let Some(org_id) = a.org_id {
        organization::Entity::find_by_id(org_id)
            .one(db)
            .await?
            .and_then(|o| o.k8s_kubeconfig)
    } else {
        None
    };
    k8s::client_from_kubeconfig(kubeconfig_b64.as_deref()).await
}

fn build_webrtc_url(base_url: &str, pod_name: &str, _namespace: &str) -> String {
    format!("{}/{}-desktop", base_url.trim_end_matches('/'), pod_name)
}

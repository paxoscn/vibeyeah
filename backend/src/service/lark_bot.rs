use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use kube::Client as K8sClient;
use open_lark::communication::im::v1::message::create::{CreateMessageBody, CreateMessageRequest};
use open_lark::communication::im::v1::message::models::ReceiveIdType;
use open_lark::communication::im::v1::message::update::{UpdateMessageBody, UpdateMessageRequest};
use open_lark::ws_client::{EventDispatcherHandler, EventHandler, LarkWsClient};
#[allow(deprecated)]
use open_lark::{Config as LarkWsConfig, CoreConfig, RequestOption};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    config::Config,
    entity::{
        org_member, organization,
        user::{self, LoginProvider},
    },
    service::{agent, lark_qr, organization as org_service},
};

// ── 飞书应用凭据（来自组织配置） ──────────────────────────────────────────────

#[derive(Clone)]
pub struct LarkAppCred {
    pub org_id: Uuid,
    pub org_name: String,
    pub app_id: String,
    pub app_secret: String,
}

// ── 事件数据结构 ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct EventEnvelope {
    header: EventHeader,
    event: EventBody,
}

#[derive(Debug, Deserialize)]
struct EventHeaderOnly {
    header: EventHeader,
}

#[derive(Debug, Deserialize)]
struct EventHeader {
    event_type: String,
    tenant_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventBody {
    sender: Sender,
    message: Message,
    #[serde(default)]
    chat: Option<Chat>,
}

#[derive(Debug, Deserialize)]
struct Sender {
    sender_id: SenderId,
}

#[derive(Debug, Deserialize)]
struct SenderId {
    open_id: String,
}

#[derive(Debug, Deserialize)]
struct Message {
    message_type: String,
    content: String,
    chat_type: String,
    #[serde(default)]
    chat_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Chat {
    chat_id: String,
}

#[derive(Debug, Deserialize)]
struct TextContent {
    text: String,
}

// ── 启动入口：遍历所有配置了飞书应用的组织 ────────────────────────────────────

pub async fn spawn_all_lark_bots(db: DatabaseConnection, k8s: K8sClient, config: Config) {
    let orgs = match organization::Entity::find()
        .filter(organization::Column::LarkAppId.is_not_null())
        .all(&db)
        .await
    {
        Ok(list) => list,
        Err(e) => {
            tracing::error!("查询组织列表失败，飞书 Bot 未启动: {e}");
            return;
        }
    };

    let enabled: Vec<_> = orgs
        .into_iter()
        .filter_map(|o| {
            let app_id = o.lark_app_id.clone()?;
            let app_secret = o.lark_app_secret.clone()?;
            if app_id.is_empty() || app_secret.is_empty() {
                return None;
            }
            Some(LarkAppCred {
                org_id: o.id,
                org_name: o.name.clone(),
                app_id,
                app_secret,
            })
        })
        .collect();

    if enabled.is_empty() {
        tracing::info!("没有组织配置了飞书应用，跳过 Bot 启动");
        return;
    }

    tracing::info!("发现 {} 个组织配置了飞书应用，逐一启动 Bot", enabled.len());

    let db = Arc::new(db);
    let k8s = Arc::new(k8s);
    let config = Arc::new(config);

    for cred in enabled {
        spawn_org_bot(Arc::clone(&db), Arc::clone(&k8s), Arc::clone(&config), cred);
    }
}

fn spawn_org_bot(
    db: Arc<DatabaseConnection>,
    k8s: Arc<K8sClient>,
    config: Arc<Config>,
    cred: LarkAppCred,
) {
    let org_name = cred.org_name.clone();
    tokio::spawn(async move {
        tracing::info!(
            "[{}] 启动飞书 Bot (app_id={}, app_secret={})",
            org_name,
            cred.app_id,
            cred.app_secret
        );
        loop {
            let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();

            let db2 = Arc::clone(&db);
            let k8s2 = Arc::clone(&k8s);
            let cfg2 = Arc::clone(&config);
            let cred2 = cred.clone();
            tokio::spawn(message_processor(rx, db2, k8s2, cfg2, cred2));

            let org_name2 = org_name.clone();
            let handler = EventDispatcherHandler::builder()
                .payload_sender(tx)
                .register_raw(
                    EventDispatcherHandler::RAW_EVENT_KEY,
                    OrgLoggingHandler {
                        org_name: org_name.clone(),
                    },
                )
                .unwrap_or_else(|e| panic!("[{org_name}] 注册事件 handler 失败: {e}"))
                .build();

            #[allow(deprecated)]
            let ws_config = LarkWsConfig::builder()
                .app_id(cred.app_id.clone())
                .app_secret(cred.app_secret.clone())
                .build()
                .unwrap_or_else(|e| panic!("[{org_name2}] 构建 WS 配置失败: {e}"));

            if let Err(e) = LarkWsClient::open(Arc::new(ws_config), handler).await {
                tracing::error!("[{}] WebSocket 断开: {e}，30s 后重连", org_name);
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    });
}

// ── 消息处理循环 ──────────────────────────────────────────────────────────────

async fn message_processor(
    mut rx: mpsc::UnboundedReceiver<Vec<u8>>,
    db: Arc<DatabaseConnection>,
    k8s: Arc<K8sClient>,
    config: Arc<Config>,
    cred: LarkAppCred,
) {
    while let Some(payload) = rx.recv().await {
        let db = Arc::clone(&db);
        let k8s = Arc::clone(&k8s);
        let cfg = Arc::clone(&config);
        let cred = cred.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_payload(&payload, &db, &k8s, &cfg, &cred).await {
                tracing::error!("[{}] 处理消息失败: {e}", cred.org_name);
            }
        });
    }
}

async fn handle_payload(
    payload: &[u8],
    db: &DatabaseConnection,
    k8s: &K8sClient,
    config: &Config,
    cred: &LarkAppCred,
) -> Result<()> {
    let header_only: EventHeaderOnly = match serde_json::from_slice(payload) {
        Ok(h) => h,
        Err(_) => return Ok(()),
    };
    if header_only.header.event_type != "im.message.receive_v1" {
        return Ok(());
    }

    let envelope: EventEnvelope = match serde_json::from_slice(payload) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("[{}] 无法解析消息事件: {e}", cred.org_name);
            return Ok(());
        }
    };

    if envelope.event.message.message_type != "text" {
        return Ok(());
    }

    let content: TextContent = match serde_json::from_str(&envelope.event.message.content) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let text = content.text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }

    let tenant_key = envelope.header.tenant_key.clone().unwrap_or_default();
    let (receive_id, receive_id_type) = resolve_target(&envelope.event);

    tracing::info!(
        "[{}] 收到消息: tenant={} text={:?}",
        cred.org_name,
        tenant_key,
        text
    );

    let sender_open_id = envelope.event.sender.sender_id.open_id.clone();
    if text.trim().replace("\\", "/") == "/add" {
        handle_add(
            db,
            k8s,
            config,
            cred,
            &sender_open_id,
            &receive_id,
            receive_id_type,
        )
        .await
    } else if is_set_command(&text) {
        handle_set(
            db,
            cred,
            &sender_open_id,
            &text,
            &receive_id,
            receive_id_type,
        )
        .await
    } else {
        handle_chat(db, cred, &text, &receive_id, receive_id_type).await
    }
}

/// 是否为 `set` 命令（`\set ...` 或 `/set ...`）。
fn is_set_command(text: &str) -> bool {
    let t = text.trim().replace("\\", "/");
    t.split_whitespace()
        .next()
        .map(|w| w.eq_ignore_ascii_case("/set"))
        .unwrap_or(false)
}

fn resolve_target(event: &EventBody) -> (String, ReceiveIdType) {
    if event.message.chat_type == "p2p" {
        return (
            event.sender.sender_id.open_id.clone(),
            ReceiveIdType::OpenId,
        );
    }
    if let Some(chat) = &event.chat {
        return (chat.chat_id.clone(), ReceiveIdType::ChatId);
    }
    if let Some(cid) = &event.message.chat_id {
        return (cid.clone(), ReceiveIdType::ChatId);
    }
    (
        event.sender.sender_id.open_id.clone(),
        ReceiveIdType::OpenId,
    )
}

// ── /add：QR 扫码注册飞书应用，成功后创建智能体 ──────────────────────────────

async fn handle_add(
    db: &DatabaseConnection,
    k8s: &K8sClient,
    config: &Config,
    cred: &LarkAppCred,
    sender_open_id: &str,
    receive_id: &str,
    receive_id_type: ReceiveIdType,
) -> Result<()> {
    let token = fetch_token(cred).await?;

    // 0. 前置校验：组织必须已配置完整的大模型（openai_*），智能体才能正常运行。
    //    任一缺失即提示联系管理员，不进入注册流程。
    let org = organization::Entity::find_by_id(cred.org_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("组织不存在: {}", cred.org_id))?;
    let mut missing = Vec::new();
    for (name, val) in [
        ("openai_base_url", org.openai_base_url.as_deref()),
        ("openai_api_key", org.openai_api_key.as_deref()),
        ("openai_model", org.openai_model.as_deref()),
    ] {
        if val.map(str::trim).unwrap_or("").is_empty() {
            missing.push(name);
        }
    }
    if !missing.is_empty() {
        let _ = send_text(
            &token,
            cred,
            receive_id,
            receive_id_type,
            &format!(
                "❌ 当前组织尚未配置完整的大模型信息，无法创建智能体。\n\
                 缺失项：{}\n\
                 请先联系组织管理员，用 /set 命令补齐后，再发送 /add。",
                missing.join("、")
            ),
        )
        .await;
        return Ok(());
    }

    let progress_id = send_text(
        &token,
        cred,
        receive_id,
        receive_id_type.clone(),
        "⏳ 正在准备注册流程...",
    )
    .await?;

    // 1. 通过 org_members.lark_open_id 查找已有成员
    let lark_user = fetch_lark_user_info(&token, sender_open_id).await;
    let now = Utc::now();

    let existing_member = org_member::Entity::find()
        .filter(org_member::Column::OrgId.eq(cred.org_id))
        .filter(org_member::Column::LarkOpenId.eq(sender_open_id))
        .one(db)
        .await?;

    let user_rec = if let Some(ref m) = existing_member {
        // 成员已存在，直接取关联用户
        user::Entity::find_by_id(m.user_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("关联用户不存在"))?
    } else {
        // 新建用户
        let new_user = user::ActiveModel {
            id: Set(Uuid::new_v4()),
            lark_union_id: Set(lark_user.as_ref().and_then(|u| u.union_id.clone())),
            nickname: Set(lark_user.as_ref().and_then(|u| u.name.clone())),
            avatar_url: Set(lark_user.as_ref().and_then(|u| u.avatar_url.clone())),
            provider: Set(LoginProvider::Lark),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        let u = new_user.insert(db).await?;
        if let Err(e) = org_service::join_default_org(db, u.id).await {
            tracing::warn!("加入默认组织失败: {e}");
        }
        u
    };

    let name = user_rec.nickname.as_deref().unwrap_or("用户").to_string();

    // 2. 确保用户是该组织成员，并写入 lark_open_id
    org_service::add_member(db, cred.org_id, user_rec.id, org_member::OrgRole::Member).await?;

    // 获取最新成员记录
    let member_rec = org_member::Entity::find()
        .filter(org_member::Column::OrgId.eq(cred.org_id))
        .filter(org_member::Column::UserId.eq(user_rec.id))
        .one(db)
        .await?;

    // 确保 lark_open_id 已写入
    if let Some(ref m) = member_rec {
        if m.lark_open_id.as_deref() != Some(sender_open_id) {
            let mut active: org_member::ActiveModel = m.clone().into();
            active.lark_open_id = Set(Some(sender_open_id.to_string()));
            active.update(db).await?;
        }
    }

    // 3. QR 注册流程
    let base_url = lark_qr::feishu_accounts_base_url();
    update_text(&token, cred, &progress_id, "⏳ 正在生成注册二维码...").await?;

    if let Err(e) = lark_qr::init(base_url).await {
        update_text(
            &token,
            cred,
            &progress_id,
            &format!("❌ 无法连接飞书注册服务：{e}"),
        )
        .await?;
        return Err(e);
    }

    let begin = lark_qr::begin(base_url).await?;

    // 4. 发送二维码
    let rid_type_str = if receive_id_type == ReceiveIdType::OpenId {
        "open_id"
    } else {
        "chat_id"
    };

    match lark_qr::qr_to_png(&begin.qr_url) {
        Ok(png_bytes) => match lark_qr::upload_image_to_lark(&token, png_bytes).await {
            Ok(image_key) => {
                update_text(
                    &token,
                    cred,
                    &progress_id,
                    &format!(
                        "📱 请用飞书扫描下方二维码或打开链接 {} 授权创建或选择专属应用。\n\
                     二维码及链接有效期 {} 秒，操作完成后系统将自动完成配置。",
                        begin.qr_url, begin.expire_in_secs
                    ),
                )
                .await?;
                let _ =
                    lark_qr::send_image_message(&token, receive_id, rid_type_str, &image_key).await;
            }
            Err(e) => {
                tracing::warn!("[lark_bot] 上传二维码失败，退回文本链接: {e}");
                update_text(
                    &token,
                    cred,
                    &progress_id,
                    &format!(
                        "📱 请在飞书中打开以下链接完成授权（有效期 {} 秒）：\n{}",
                        begin.expire_in_secs, begin.qr_url
                    ),
                )
                .await?;
            }
        },
        Err(e) => {
            tracing::warn!("[lark_bot] 生成二维码失败，退回文本链接: {e}");
            update_text(
                &token,
                cred,
                &progress_id,
                &format!(
                    "📱 请在飞书中打开以下链接完成授权（有效期 {} 秒）：\n{}",
                    begin.expire_in_secs, begin.qr_url
                ),
            )
            .await?;
        }
    }

    // 5. 轮询扫码结果
    let poll = match lark_qr::poll(
        base_url,
        &begin.device_code,
        begin.interval_secs,
        begin.expire_in_secs,
    )
    .await?
    {
        Some(p) => p,
        None => {
            update_text(
                &token,
                cred,
                &progress_id,
                "❌ 二维码已过期或授权被拒绝，请重新发送 /add 重试。",
            )
            .await?;
            return Ok(());
        }
    };

    // let _ = send_text(&token, cred, receive_id, receive_id_type.clone(), "搞定！").await;

    // 6. 凭据直接在创建 agent 时写入（见 create_agent_step），此处只需记录日志
    update_text(
        &token,
        cred,
        &progress_id,
        &format!("✅ 飞书应用授权成功！\nApp ID: {}", poll.app_id),
    )
    .await?;

    // probe Bot 信息（open_id / name），之后写入 agent
    let bot_info = lark_qr::probe_bot(&poll.app_id, &poll.app_secret).await;

    // 7. 创建智能体，携带飞书配置
    create_agent_step(
        db,
        k8s,
        config,
        cred,
        &user_rec,
        &name,
        &token,
        &progress_id,
        poll.app_id.clone(),
        poll.app_secret.clone(),
        bot_info.as_ref().map(|b| b.open_id.clone()),
        bot_info.as_ref().map(|b| b.name.clone()),
    )
    .await
}

/// 最后一步：创建智能体，并启动后台 task 轮询 Pod 状态，通过飞书消息告知用户
#[allow(clippy::too_many_arguments)]
async fn create_agent_step(
    db: &DatabaseConnection,
    k8s: &K8sClient,
    config: &Config,
    cred: &LarkAppCred,
    user_rec: &crate::entity::user::Model,
    name: &str,
    token: &str,
    progress_id: &str,
    lark_app_id: String,
    lark_app_secret: String,
    lark_bot_open_id: Option<String>,
    lark_bot_name: Option<String>,
) -> Result<()> {
    update_text(
        token,
        cred,
        progress_id,
        &format!("⏳ 正在为 {name} 创建智能体..."),
    )
    .await?;

    // 一个用户可以创建多个智能体，第二个起加序号避免重名
    use crate::entity::agent as agent_entity;
    let existing_count = agent_entity::Entity::find()
        .filter(agent_entity::Column::CreatedBy.eq(user_rec.id))
        .filter(agent_entity::Column::OrgId.eq(cred.org_id))
        .count(db)
        .await
        .unwrap_or(0);
    let agent_name = if existing_count == 0 {
        format!("{} 的智能体", name)
    } else {
        format!("{} 的智能体 #{}", name, existing_count + 1)
    };

    let input = agent::CreateAgentInput {
        name: agent_name,
        description: Some(format!("由飞书 /add 自动创建，组织：{}", cred.org_name)),
        system_prompt: None,
        user_id: user_rec.id,
        org_id: cred.org_id,
        lark_app_id: Some(lark_app_id),
        lark_app_secret: Some(lark_app_secret),
        lark_bot_open_id: lark_bot_open_id.clone(),
        lark_bot_name: lark_bot_name.clone(),
        wechat_account_id: None,
        wechat_token: None,
        wechat_base_url: None,
        wechat_user_id: None,
    };

    let agent_rec = match agent::create_agent(db, k8s, config, input).await {
        Ok(a) => {
            update_text(
                token,
                cred,
                progress_id,
                &format!(
                    "✅ 智能体已提交创建！\n名称：{}\n组织：{}\n容器启动中，请稍候...",
                    a.name, cred.org_name,
                ),
            )
            .await?;
            a
        }
        Err(e) => {
            update_text(token, cred, progress_id, &format!("❌ 创建智能体失败：{e}")).await?;
            return Err(e);
        }
    };

    // 后台轮询 Pod 状态，就绪后通过飞书消息告知
    let pod_name = match agent_rec.pod_name.clone() {
        Some(p) => p,
        None => {
            update_text(
                token,
                cred,
                progress_id,
                "⚠️ 容器信息缺失，无法追踪启动状态",
            )
            .await?;
            return Ok(());
        }
    };
    let ns = agent_rec
        .pod_namespace
        .clone()
        .unwrap_or_else(|| "default".to_string());

    // 克隆需要跨 await 用的值
    let db = db.clone();
    let k8s = k8s.clone();
    let cred = cred.clone();
    let token = token.to_string();
    let progress_id = progress_id.to_string();
    let agent_id = agent_rec.id;

    tokio::spawn(async move {
        tracing::info!("[lark_bot] 开始轮询 Pod 状态: pod={} ns={}", pod_name, ns);
        poll_and_notify(
            &db,
            &k8s,
            &cred,
            &token,
            &progress_id,
            agent_id,
            &pod_name,
            &ns,
            lark_bot_open_id.clone(),
            lark_bot_name.clone(),
        )
        .await;
    });

    Ok(())
}

/// 轮询 Pod 状态并通过飞书更新消息，最终写库
async fn poll_and_notify(
    db: &DatabaseConnection,
    k8s: &K8sClient,
    cred: &LarkAppCred,
    token: &str,
    progress_id: &str,
    agent_id: uuid::Uuid,
    pod_name: &str,
    namespace: &str,
    lark_bot_open_id: Option<String>,
    lark_bot_name: Option<String>,
) {
    use crate::entity::{agent as agent_entity, organization};
    use crate::service::k8s::{client_from_kubeconfig, wait_for_pod_ready, WaitResult};
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};

    // 用组织专属 kubeconfig 构建 client，失败则降级使用传入的默认 client
    let org_client = {
        let kubeconfig = organization::Entity::find_by_id(cred.org_id)
            .one(db)
            .await
            .ok()
            .flatten()
            .and_then(|o| o.k8s_kubeconfig);
        match client_from_kubeconfig(kubeconfig.as_deref()).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("[lark_bot] 构建组织 K8s client 失败，使用默认: {e}");
                k8s.clone()
            }
        }
    };

    // 最多等 10 分钟，每 10 秒查一次
    let result = wait_for_pod_ready(&org_client, pod_name, namespace, 600, 10).await;

    let bot_mention = lark_bot_name
        .as_deref()
        .map(|name| {
            format!(
                "<at user_id=\"{}\">{}</at>",
                lark_bot_open_id.as_deref().unwrap_or(""),
                name
            )
        })
        .unwrap_or_else(|| "机器人".to_string());

    let (new_status, msg) = match result {
        WaitResult::Running => (
            agent_entity::PodStatus::Running,
            format!("🎉 智能体容器已就绪！\n组织：{}\n开始与 {} 聊天吧。", cred.org_name, bot_mention),
        ),
        WaitResult::Failed(events) => (
            agent_entity::PodStatus::Failed,
            format!("❌ 容器启动失败\n\n异常事件：\n{events}\n\n请联系管理员或重新发送 /add 重试。"),
        ),
        WaitResult::Timeout => (
            agent_entity::PodStatus::Unknown,
            "⏱️ 容器启动超时（10 分钟），当前状态未知。\n可能仍在调度中，请稍后登录平台查看，或联系管理员。".to_string(),
        ),
    };

    // 更新数据库
    if let Ok(Some(a)) = agent_entity::Entity::find_by_id(agent_id).one(db).await {
        let mut active: agent_entity::ActiveModel = a.into();
        active.pod_status = Set(new_status);
        active.pod_synced_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        if let Err(e) = active.update(db).await {
            tracing::error!("[lark_bot] 更新 agent 状态失败: {e}");
        }
    }

    // 通知用户：token 可能已过期，重新获取
    let fresh_token = fetch_token(cred).await.unwrap_or_else(|e| {
        tracing::warn!("[lark_bot] 获取 token 失败，使用原 token: {e}");
        token.to_string()
    });

    if let Err(e) = update_text(&fresh_token, cred, progress_id, &msg).await {
        tracing::warn!("[lark_bot] 发送 Pod 状态通知失败: {e}");
    }
}

// ── 普通对话 ──────────────────────────────────────────────────────────────────

async fn handle_chat(
    db: &DatabaseConnection,
    cred: &LarkAppCred,
    text: &str,
    receive_id: &str,
    receive_id_type: ReceiveIdType,
) -> Result<()> {
    let token = fetch_token(cred).await?;
    let msg_id = send_text(&token, cred, receive_id, receive_id_type, "💭 思考中...").await?;

    // 组织级 LLM 配置（openai_* 已存于 organizations 表，默认未配置）
    let org = organization::Entity::find_by_id(cred.org_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("组织不存在: {}", cred.org_id))?;

    let reply = match org.openai_base_url.as_deref() {
        Some(base) if !base.trim().is_empty() => {
            let key = org.openai_api_key.as_deref().unwrap_or("");
            let model = org.openai_model.as_deref().unwrap_or("");
            call_llm(base, key, model, text).await.unwrap_or_else(|e| {
                tracing::error!("[{}] LLM 失败: {e}", cred.org_name);
                format!("抱歉，出现错误：{e}")
            })
        }
        _ => "该组织尚未配置大模型（openai_base_url 等为空），无法直接对话。\n\
              如需使用专属智能体，请发送 /add 创建。"
            .to_string(),
    };

    update_text(&token, cred, &msg_id, &reply).await?;
    Ok(())
}

// ── set 命令：Owner 设置组织属性 ─────────────────────────────────────────────

async fn handle_set(
    db: &DatabaseConnection,
    cred: &LarkAppCred,
    sender_open_id: &str,
    text: &str,
    receive_id: &str,
    receive_id_type: ReceiveIdType,
) -> Result<()> {
    let token = fetch_token(cred).await?;

    let parts: Vec<&str> = text.trim().split_whitespace().collect();
    // parts[0] == "/set"
    if parts.len() < 3 {
        let _ = send_text(
            &token,
            cred,
            receive_id,
            receive_id_type,
            "用法：/set <key> <value>\n\
             支持的 key：openai_base_url、openai_api_key、openai_model\n\
             示例：set openai_base_url https://api.openai.com\n\
             （openai_base_url 请勿带 /v1 尾缀，系统会按需要自动补全）",
        )
        .await;
        return Ok(());
    }
    let key = parts[1];
    let value = parts[2..].join(" ");

    // 校验：必须是组织 Owner
    let member = org_member::Entity::find()
        .filter(org_member::Column::OrgId.eq(cred.org_id))
        .filter(org_member::Column::LarkOpenId.eq(sender_open_id))
        .one(db)
        .await?;
    let member = match member {
        Some(m) => m,
        None => {
            let _ = send_text(
                &token,
                cred,
                receive_id,
                receive_id_type,
                "无法识别你的成员身份，请先发送 /add 加入组织",
            )
            .await;
            return Ok(());
        }
    };
    if !matches!(member.role, org_member::OrgRole::Owner) {
        let _ = send_text(
            &token,
            cred,
            receive_id,
            receive_id_type,
            "只有组织的 Owner 才能设置组织属性",
        )
        .await;
        return Ok(());
    }

    match org_service::set_org_property(db, cred.org_id, key, &value).await {
        Ok(()) => {
            let _ = send_text(
                &token,
                cred,
                receive_id,
                receive_id_type,
                &format!("✅ 已设置 {key} = {value}"),
            )
            .await;
        }
        Err(e) => {
            let _ = send_text(
                &token,
                cred,
                receive_id,
                receive_id_type,
                &format!("设置失败：{e}"),
            )
            .await;
        }
    }
    Ok(())
}

// ── openlark API 封装 ─────────────────────────────────────────────────────────

/// 获取 tenant_access_token
/// 飞书响应格式（顶层平铺）：{ "code": 0, "tenant_access_token": "xxx", "expire": 7200 }
async fn fetch_token(cred: &LarkAppCred) -> Result<String> {
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
        .json(&json!({ "app_id": cred.app_id, "app_secret": cred.app_secret }))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("[{}] 获取 token 请求失败: {e}", cred.org_name))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("[{}] 解析 token 响应失败: {e}", cred.org_name))?;

    let code = resp["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(anyhow::anyhow!(
            "[{}] 获取 token 失败 code={} msg={}",
            cred.org_name,
            code,
            resp["msg"].as_str().unwrap_or("unknown")
        ));
    }

    resp["tenant_access_token"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("[{}] 响应缺少 tenant_access_token", cred.org_name))
}

async fn send_text(
    token: &str,
    cred: &LarkAppCred,
    receive_id: &str,
    receive_id_type: ReceiveIdType,
    text: &str,
) -> Result<String> {
    let body = CreateMessageBody {
        receive_id: receive_id.to_string(),
        msg_type: "text".to_string(),
        content: json!({ "text": text }).to_string(),
        uuid: None,
    };
    let option = RequestOption::builder()
        .tenant_access_token(token.to_string())
        .build();
    let resp: serde_json::Value = CreateMessageRequest::new(build_core(cred))
        .receive_id_type(receive_id_type)
        .execute_with_options(body, option)
        .await
        .map_err(|e| anyhow::anyhow!("发送消息失败: {e:?}"))?;
    Ok(resp["message_id"].as_str().unwrap_or_default().to_string())
}

async fn update_text(token: &str, cred: &LarkAppCred, message_id: &str, text: &str) -> Result<()> {
    if message_id.is_empty() {
        return Ok(());
    }
    let body = UpdateMessageBody {
        msg_type: "text".to_string(),
        content: json!({ "text": text }).to_string(),
    };
    let option = RequestOption::builder()
        .tenant_access_token(token.to_string())
        .build();
    UpdateMessageRequest::new(build_core(cred))
        .message_id(message_id.to_string())
        .execute_with_options(body, option)
        .await
        .map_err(|e| anyhow::anyhow!("更新消息失败: {e:?}"))?;
    Ok(())
}

struct LarkUserBasic {
    union_id: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

async fn fetch_lark_user_info(token: &str, open_id: &str) -> Option<LarkUserBasic> {
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!(
            "https://open.feishu.cn/open-apis/contact/v3/users/{open_id}"
        ))
        .query(&[("user_id_type", "open_id")])
        .bearer_auth(token)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let user = &resp["data"]["user"];
    Some(LarkUserBasic {
        union_id: user["union_id"].as_str().map(str::to_string),
        name: user["name"].as_str().map(str::to_string),
        avatar_url: user["avatar"]["avatar_240"].as_str().map(str::to_string),
    })
}

fn build_core(cred: &LarkAppCred) -> CoreConfig {
    CoreConfig::builder()
        .app_id(cred.app_id.clone())
        .app_secret(cred.app_secret.clone())
        .enable_token_cache(false)
        .req_timeout(Duration::from_secs(15))
        .build()
}

// ── LLM 调用 ──────────────────────────────────────────────────────────────────

async fn call_llm(
    openai_base_url: &str,
    openai_api_key: &str,
    openai_model: &str,
    text: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    // openai_base_url 约定不含 /v1 尾缀；旧值若带了 /v1 则不重复拼接，
    // 保证与 .hermes/config.yaml / .env 里的 base_url 语义一致（那两处由模板补 /v1）。
    let base = openai_base_url.trim_end_matches('/');
    let base = if base.ends_with("/v1") {
        base.to_string()
    } else {
        format!("{base}/v1")
    };
    let resp: serde_json::Value = client
        .post(format!("{base}/chat/completions"))
        .bearer_auth(openai_api_key)
        .json(&json!({
            "model": openai_model,
            "messages": [
                {
                    "role": "system",
                    "content": "你是 VibeYeah 平台的 AI 助手，用简洁友好的中文回答用户问题。\n\
                                你知道以下事实:\n\
                                * 用户可以使用/add命令创建自己的智能体，该智能体背后是一台安装了Hermes和浏览器、Claude Code等工具的远程桌面，每次/add都会创建一个新的智能体（各自对应独立的飞书机器人）\n\
                                * Hermes会自动存储与用户相关的记忆，如需导出请联系管理员\n\n\
                                注意只有/add命令能创建智能体，用户不能通过自然语言要求你创建或管理智能体"
                },
                { "role": "user", "content": text }
            ],
            "max_tokens": 1024
        }))
        .send()
        .await?
        .json()
        .await?;

    resp["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("LLM 响应格式异常: {:?}", resp))
}

// ── 日志 Handler ──────────────────────────────────────────────────────────────

struct OrgLoggingHandler {
    org_name: String,
}

impl EventHandler for OrgLoggingHandler {
    fn handle(
        &self,
        payload: &[u8],
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event_type = serde_json::from_slice::<EventHeaderOnly>(payload)
            .ok()
            .map(|e| e.header.event_type)
            .unwrap_or_else(|| "unknown".to_string());
        tracing::debug!(
            "[{}] WS 事件: type={} size={} bytes",
            self.org_name,
            event_type,
            payload.len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::is_set_command;

    #[test]
    fn detects_set_command() {
        assert!(is_set_command("/set openai_base_url https://x"));
        assert!(is_set_command("/set openai_base_url"));
        assert!(is_set_command("/SET openai_base_url x"));
        assert!(is_set_command("/set openai_base_url x"));
        assert!(is_set_command("  /set openai_base_url x"));
        assert!(!is_set_command("/add"));
        assert!(!is_set_command("hello"));
        assert!(is_set_command("/set"));
        assert!(is_set_command("\\set"));
        assert!(!is_set_command("set"));
        assert!(!is_set_command("/setting something"));
        assert!(!is_set_command(""));
    }
}

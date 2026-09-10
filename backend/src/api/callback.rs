use axum::{
    body::Bytes,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};

use crate::{service::callback as cb, AppState};

/// GET/POST /callback/{skill}/{user_id}?foobar=...
///
/// 外部 agent 调用路由：
/// 1. 在 NAS 上找到为该 `user_id` 安装了 `skill` 的所有 agent；
/// 2. 命中即记录日志并返回成功（202），随后后台以该用户身份调用 agent pod 内的 hermes；
/// 3. 未命中任何 agent 时返回 404。
pub async fn callback(
    Extension(state): Extension<AppState>,
    Path((skill, user_id)): Path<(String, String)>,
    Query(params): Query<Vec<(String, String)>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // 基础校验：限定为安全字符集（这些值会拼进 NAS 路径与 pod 内 shell 命令）
    let invalid = |s: &str| {
        s.is_empty()
            || s.chars()
                .any(|c| !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_' | '.'))
    };
    if invalid(&skill) || invalid(&user_id) {
        tracing::warn!(
            "[callback] 非法 skill/user_id: skill={} user={}",
            skill,
            user_id
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({"error": "非法的 skill 或 user_id（仅允许字母、数字、-、_、.）"}),
            ),
        );
    }

    // 可选共享密钥校验：仅当配置了 callback_token 时生效
    if !state.config.callback_token.is_empty() {
        let provided = headers
            .get("x-callback-token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .or_else(|| {
                params
                    .iter()
                    .find(|(k, _)| k == "token")
                    .map(|(_, v)| v.clone())
            });
        if provided.as_deref() != Some(state.config.callback_token.as_str()) {
            tracing::warn!("[callback] token 校验失败 skill={} user={}", skill, user_id);
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "callback token 无效"})),
            );
        }
    }

    // 扫描 NAS（阻塞式文件 IO，放到阻塞线程执行）
    let nas_root = state.config.nas_mount_root.clone();
    let scan_user = user_id.clone();
    let scan_skill = skill.clone();
    let agent_dirs = match tokio::task::spawn_blocking(move || {
        cb::find_agents_with_skill(&nas_root, &scan_user, &scan_skill)
    })
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("[callback] NAS 扫描任务异常: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "NAS 扫描失败"})),
            );
        }
    };

    if agent_dirs.is_empty() {
        tracing::warn!(
            "[callback] 未找到安装该技能的 agent skill={} user={}",
            skill,
            user_id
        );
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "未找到安装了该技能的 agent"})),
        );
    }

    tracing::info!(
        "[callback] 命中 {} 个 agent: {:?} skill={} user={}",
        agent_dirs.len(),
        agent_dirs,
        skill,
        user_id
    );

    // 构造回调上下文，后台异步调用 hermes（失败继续遍历，成功即止）
    // 用于鉴权的 token 参数不转发给 hermes
    let forward_params: Vec<(String, String)> = if !state.config.callback_token.is_empty() {
        params.into_iter().filter(|(k, _)| k != "token").collect()
    } else {
        params
    };

    let body_text = String::from_utf8_lossy(&body).to_string();
    let input = cb::CallbackInput {
        skill: skill.clone(),
        user_id: user_id.clone(),
        query: forward_params,
        body: if body_text.trim().is_empty() {
            None
        } else {
            Some(body_text)
        },
    };

    let db = state.db.clone();
    let exec_timeout_secs = state.config.hermes_exec_timeout_secs;
    let agents_resp = agent_dirs.clone();
    tokio::spawn(async move {
        cb::dispatch(&db, exec_timeout_secs, agent_dirs, input).await;
    });

    // 命中即返回成功
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "status": "accepted",
            "message": "已受理，正在调用 agent",
            "skill": skill,
            "user_id": user_id,
            "agents": agents_resp,
        })),
    )
}

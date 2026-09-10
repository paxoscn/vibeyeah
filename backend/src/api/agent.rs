use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    middleware::auth::CurrentUser,
    service::agent::{self, CreateAgentInput},
    service::organization,
    AppState,
};

/// 获取默认组织的 ID。单组织模式下所有 agent 都属于默认组织。
async fn default_org_id(_state: &AppState) -> Result<Uuid, (StatusCode, Json<serde_json::Value>)> {
    let default_id: Uuid = organization::DEFAULT_ORG_ID.parse().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "默认组织 ID 解析失败"})),
        )
    })?;
    Ok(default_id)
}

#[derive(Deserialize)]
pub struct CreateAgentReq {
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
}

/// POST /api/agents — 在默认组织下创建智能体
pub async fn create_agent(
    Extension(state): Extension<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(body): Json<CreateAgentReq>,
) -> impl IntoResponse {
    let org_id = match default_org_id(&state).await {
        Ok(id) => id,
        Err(r) => return r,
    };

    let input = CreateAgentInput {
        name: body.name,
        description: body.description,
        system_prompt: body.system_prompt,
        user_id: current_user.id,
        org_id,
        lark_app_id: None,
        lark_app_secret: None,
        lark_bot_open_id: None,
        lark_bot_name: None,
        wechat_account_id: None,
        wechat_token: None,
        wechat_base_url: None,
        wechat_user_id: None,
    };

    match agent::create_agent(&state.db, &state.k8s_client, &state.config, input).await {
        Ok(a) => (StatusCode::CREATED, Json(serde_json::json!(a))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// GET /api/agents — 列出默认组织下的所有智能体
pub async fn list_agents(
    Extension(state): Extension<AppState>,
    Extension(current_user): Extension<CurrentUser>,
) -> impl IntoResponse {
    let org_id = match default_org_id(&state).await {
        Ok(id) => id,
        Err(r) => return r,
    };

    // 鉴权：必须是组织成员
    match organization::get_member(&state.db, &org_id, &current_user.id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "无权限"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    }

    match agent::list_org_agents(&state.db, &org_id).await {
        Ok(list) => (StatusCode::OK, Json(serde_json::json!(list))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// GET /api/orgs/:org_id/agents — 列出指定组织下的所有智能体
pub async fn list_org_agents(
    Extension(state): Extension<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Path(org_id): Path<Uuid>,
) -> impl IntoResponse {
    // 鉴权：必须是组织成员
    match organization::get_member(&state.db, &org_id, &current_user.id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "无权限"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            );
        }
    }

    match agent::list_org_agents(&state.db, &org_id).await {
        Ok(list) => (StatusCode::OK, Json(serde_json::json!(list))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// POST /api/agents/:id/stream/start
pub async fn start_stream(
    Extension(state): Extension<AppState>,
    Extension(_current_user): Extension<CurrentUser>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match agent::start_stream(&state.db, &agent_id).await {
        Ok(a) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "stream_active": a.stream_active,
                "webrtc_url": a.webrtc_url,
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// POST /api/agents/:id/stream/stop
pub async fn stop_stream(
    Extension(state): Extension<AppState>,
    Extension(_current_user): Extension<CurrentUser>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match agent::stop_stream(&state.db, &agent_id).await {
        Ok(a) => (
            StatusCode::OK,
            Json(serde_json::json!({"stream_active": a.stream_active})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    middleware::auth::CurrentUser,
    service::{self, agent::CreateAgentInput, organization, wechat_qr},
    AppState,
};

#[derive(Deserialize)]
pub struct QrBeginReq {
    #[serde(default = "default_bot_type")]
    pub bot_type: String,
}

fn default_bot_type() -> String {
    "3".to_string()
}

pub async fn qr_begin(
    Extension(_current_user): Extension<CurrentUser>,
    Json(body): Json<QrBeginReq>,
) -> impl IntoResponse {
    match wechat_qr::get_qr_code(&body.bot_type).await {
        Ok((qrcode, qrcode_url)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "qrcode": qrcode,
                "qrcode_url": qrcode_url,
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(Deserialize)]
pub struct QrPollReq {
    pub qrcode: String,
}

pub async fn qr_poll(
    Extension(_current_user): Extension<CurrentUser>,
    Json(body): Json<QrPollReq>,
) -> impl IntoResponse {
    match wechat_qr::poll_qr_status(&body.qrcode).await {
        Ok(status) => {
            let status_str = status.status.as_deref().unwrap_or("wait");
            let mut resp = serde_json::json!({ "status": status_str });

            if status_str == "scaned_but_redirect" {
                if let Some(host) = &status.redirect_host {
                    resp["redirect_host"] = serde_json::json!(host);
                }
            }

            if status_str == "confirmed" {
                if let Some(creds) = wechat_qr::extract_credentials(&status) {
                    resp["credentials"] = serde_json::json!({
                        "account_id": creds.account_id,
                        "token": creds.token,
                        "base_url": creds.base_url,
                        "user_id": creds.user_id,
                    });
                } else {
                    resp["status"] = serde_json::json!("error");
                    resp["error"] = serde_json::json!("凭证不完整");
                }
            }

            (StatusCode::OK, Json(resp))
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(Deserialize)]
pub struct CreateWechatAgentReq {
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub wechat_account_id: String,
    pub wechat_token: String,
    pub wechat_base_url: String,
    pub wechat_user_id: String,
}

/// POST /api/wechat/agents — 在默认组织下创建微信智能体
pub async fn create_wechat_agent(
    Extension(state): Extension<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(body): Json<CreateWechatAgentReq>,
) -> impl IntoResponse {
    let org_id: Uuid = match organization::DEFAULT_ORG_ID.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "默认组织 ID 解析失败"})),
            );
        }
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
        wechat_account_id: Some(body.wechat_account_id),
        wechat_token: Some(body.wechat_token),
        wechat_base_url: Some(body.wechat_base_url),
        wechat_user_id: Some(body.wechat_user_id),
    };

    match service::agent::create_agent(&state.db, &state.k8s_client, &state.config, input).await {
        Ok(a) => (StatusCode::CREATED, Json(serde_json::json!(a))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

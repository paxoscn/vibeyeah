use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;

use crate::{service::auth, AppState};

#[derive(Deserialize)]
pub struct SendCodeReq {
    pub phone: String,
}

#[derive(Deserialize)]
pub struct PhoneLoginReq {
    pub phone: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct LarkLoginReq {
    pub code: String,
}

/// POST /api/auth/send-code
pub async fn send_code(
    Extension(state): Extension<AppState>,
    Json(body): Json<SendCodeReq>,
) -> impl IntoResponse {
    match auth::send_phone_code(&state.db, &body.phone).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => {
            tracing::error!("send_code error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

/// POST /api/auth/phone-login
pub async fn phone_login(
    Extension(state): Extension<AppState>,
    Json(body): Json<PhoneLoginReq>,
) -> impl IntoResponse {
    match auth::login_by_phone(&state.db, &state.config, &body.phone, &body.code).await {
        Ok(token) => (StatusCode::OK, Json(serde_json::json!({"token": token}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// POST /api/auth/lark-login
pub async fn lark_login(
    Extension(state): Extension<AppState>,
    Json(body): Json<LarkLoginReq>,
) -> impl IntoResponse {
    match auth::login_by_lark(&state.db, &state.config, &body.code).await {
        Ok(token) => (StatusCode::OK, Json(serde_json::json!({"token": token}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

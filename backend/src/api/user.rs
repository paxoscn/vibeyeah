use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use sea_orm::EntityTrait;

use crate::{entity::user, middleware::auth::CurrentUser, AppState};

/// GET /api/user/me  （需要登录）
pub async fn me(
    Extension(state): Extension<AppState>,
    Extension(current_user): Extension<CurrentUser>,
) -> impl IntoResponse {
    match user::Entity::find_by_id(current_user.id)
        .one(&state.db)
        .await
    {
        Ok(Some(u)) => (StatusCode::OK, Json(serde_json::json!(u))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "用户不存在"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

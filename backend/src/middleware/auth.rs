use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{service::auth::verify_jwt, AppState};

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = verify_jwt(token, &state.config).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = claims
        .sub
        .parse::<Uuid>()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(CurrentUser { id: user_id });
    Ok(next.run(req).await)
}

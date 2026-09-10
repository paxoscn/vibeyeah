use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    middleware::auth::CurrentUser,
    service::{organization, sync as sync_service},
    AppState,
};

#[derive(Deserialize)]
pub struct GitSyncReq {
    /// 需要同步的路径列表，如 ["/home/agent/.hermes/SOUL.md", "/home/agent/.hermes/memories"]
    pub paths: Vec<String>,
    /// 可选：提交信息，默认 "chore: sync agent state"
    pub commit_message: Option<String>,
}

/// POST /api/orgs/:org_id/agents/git-sync — 对指定组织下的所有 agent 进行 git 同步
pub async fn git_sync_org_agents(
    Extension(state): Extension<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Path(org_id): Path<Uuid>,
    Json(body): Json<GitSyncReq>,
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

    if body.paths.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "paths 不能为空"})),
        );
    }

    let input = sync_service::GitSyncInput {
        org_id,
        paths: body.paths,
        commit_message: body.commit_message,
    };
    // 异步触发，立即返回 202
    tokio::spawn(async move {
        if let Err(e) = sync_service::git_sync_org_agents(&state.db, input).await {
            tracing::error!("git_sync_org_agents 失败: {}", e);
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"message": "同步任务已提交，正在后台执行"})),
    )
}

/// POST /api/agents/git-sync — 对默认组织下的所有 agent 进行 git 同步
pub async fn git_sync_default_org(
    Extension(state): Extension<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(body): Json<GitSyncReq>,
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

    if body.paths.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "paths 不能为空"})),
        );
    }

    let input = sync_service::GitSyncInput {
        org_id,
        paths: body.paths,
        commit_message: body.commit_message,
    };
    // 异步触发，立即返回 202
    tokio::spawn(async move {
        if let Err(e) = sync_service::git_sync_org_agents(&state.db, input).await {
            tracing::error!("git_sync_org_agents 失败: {}", e);
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"message": "同步任务已提交，正在后台执行"})),
    )
}

//! VibeYeah 核心库
//!
//! 提供所有实体模型、服务层、配置、迁移以及核心路由构建器。
//! 既可支撑本仓库的 `vibeyeah` 可执行文件，也可作为依赖被其他服务复用。

pub mod api;
pub mod config;
pub mod entity;
pub mod middleware;
pub mod migration;
pub mod service;

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Extension, Router,
};
use kube::Client;
use sea_orm::DatabaseConnection;

use config::Config;

/// 应用全局共享状态
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Config,
    pub k8s_client: Client,
}

/// 构建核心路由（受保护路由 + 公开路由）。
///
/// 返回的 Router 不含状态类型参数（`Router<()>`），
/// 调用方可以在此之上 `.merge()` 自己的路由。
/// 所有处理器通过 `Extension<AppState>` 获取状态。
pub fn core_router(state: AppState) -> Router {
    // 受保护路由（需要 JWT）
    let protected = Router::new()
        // 用户
        .route("/api/user/me", get(api::user::me))
        // 智能体（单组织模式，无需 org_id 参数）
        .route("/api/agents", post(api::agent::create_agent))
        .route("/api/agents", get(api::agent::list_agents))
        .route(
            "/api/agents/{id}/stream/start",
            post(api::agent::start_stream),
        )
        .route(
            "/api/agents/{id}/stream/stop",
            post(api::agent::stop_stream),
        )
        // Git 同步（对默认组织）
        .route(
            "/api/agents/git-sync",
            post(api::sync::git_sync_default_org),
        )
        // 微信
        .route("/api/wechat/qr/begin", post(api::wechat::qr_begin))
        .route("/api/wechat/qr/poll", post(api::wechat::qr_poll))
        .route("/api/wechat/agents", post(api::wechat::create_wechat_agent))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::require_auth,
        ));

    // 公开路由
    let public = Router::new()
        .route("/api/auth/send-code", post(api::auth::send_code))
        .route("/api/auth/phone-login", post(api::auth::phone_login))
        .route("/api/auth/lark-login", post(api::auth::lark_login))
        // 外部 agent 调用路由：按技能与用户回调，转发到 agent pod 内的 hermes
        .route(
            "/callback/{skill}/{user_id}",
            get(api::callback::callback).post(api::callback::callback),
        )
        .route("/health", get(|| async { "ok" }));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(Extension(state))
}

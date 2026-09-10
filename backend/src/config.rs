use anyhow::{anyhow, Context, Result};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::collections::HashMap;

use crate::entity::setting;

/// 平台配置。除 `database_url` 与 `bind_addr`（来自环境变量）外，
/// 其余字段在启动时从数据库 `settings` 表加载（见 `load_from_db`）。
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub jwt_secret: String,
    pub jwt_expire_hours: i64,
    /// 全局飞书 OAuth 应用（用于网页登录），可选
    pub lark_app_id: String,
    pub lark_app_secret: String,
    pub k8s_namespace: String,
    pub pod_sync_interval_secs: u64,
    /// NAS 在 backend 容器内的挂载根目录（与 K8s Pod 保持一致）
    pub nas_mount_root: String,
    /// 回调路由在 pod 内 oneshot 调用 hermes 的超时（秒）
    pub hermes_exec_timeout_secs: u64,
    /// 外部回调接口的可选共享密钥；为空则不校验
    pub callback_token: String,
    /// agent 桌面镜像
    pub desktop_image: String,
    /// WebRTC sidecar 镜像
    pub sidecar_image: String,
    /// 挂载进 agent pod 的共享 NAS PVC 名称
    pub nas_pvc_name: String,
    /// 桌面实时观看的基础 URL
    pub webrtc_base_url: String,
}

/// 各配置项的默认值（与 `settings` 表种子默认一致）
fn defaults() -> Config {
    Config {
        database_url: String::new(),
        bind_addr: String::new(),
        jwt_secret: String::new(),
        jwt_expire_hours: 72,
        lark_app_id: String::new(),
        lark_app_secret: String::new(),
        k8s_namespace: "default".into(),
        pod_sync_interval_secs: 30,
        nas_mount_root: "/data/nas".into(),
        hermes_exec_timeout_secs: 900,
        callback_token: String::new(),
        desktop_image: "vibeyeah/desktop:latest".into(),
        sidecar_image: "vibeyeah/sidecar:latest".into(),
        nas_pvc_name: "vibeyeah-nas-pvc".into(),
        webrtc_base_url: "http://localhost:8889".into(),
    }
}

impl Config {
    /// 启动引导：仅读取环境变量（DATABASE_URL、BIND_ADDR 可选）。
    /// `DATABASE_URL` 为空时默认使用本地 SQLite 文件（首次连接自动创建）；
    /// 其余字段先用默认值占位，连接并迁移数据库后由 `load_from_db` 覆盖。
    pub fn bootstrap() -> Result<Self> {
        let mut c = defaults();
        let url = std::env::var("DATABASE_URL").unwrap_or_default();
        c.database_url = if url.trim().is_empty() {
            tracing::info!("DATABASE_URL 未设置，使用默认 SQLite 文件 sqlite://vibeyeah.db");
            "sqlite://vibeyeah.db".to_string()
        } else {
            url
        };
        c.bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        Ok(c)
    }

    /// 从 `settings` 表一次性加载平台配置（除 DATABASE_URL/BIND_ADDR 外的所有项）。
    /// 缺失键回退默认值；`jwt_secret` 为空则报错（保持其“必填”语义）。
    pub async fn load_from_db(&mut self, db: &DatabaseConnection) -> Result<()> {
        let rows = setting::Entity::find()
            .all(db)
            .await
            .context("读取 settings 配置表失败")?;
        let map: HashMap<String, String> = rows.into_iter().map(|r| (r.key, r.value)).collect();

        let get = |k: &str| map.get(k).cloned();

        if let Some(v) = get("jwt_secret") {
            self.jwt_secret = v;
        }
        if let Some(v) = get("jwt_expire_hours") {
            self.jwt_expire_hours = v.parse().unwrap_or(72);
        }
        if let Some(v) = get("lark_app_id") {
            self.lark_app_id = v;
        }
        if let Some(v) = get("lark_app_secret") {
            self.lark_app_secret = v;
        }
        if let Some(v) = get("k8s_namespace") {
            self.k8s_namespace = v;
        }
        if let Some(v) = get("pod_sync_interval_secs") {
            self.pod_sync_interval_secs = v.parse().unwrap_or(30);
        }
        if let Some(v) = get("nas_mount_root") {
            self.nas_mount_root = v;
        }
        if let Some(v) = get("hermes_exec_timeout_secs") {
            self.hermes_exec_timeout_secs = v.parse().unwrap_or(900);
        }
        if let Some(v) = get("callback_token") {
            self.callback_token = v;
        }
        if let Some(v) = get("desktop_image") {
            self.desktop_image = v;
        }
        if let Some(v) = get("sidecar_image") {
            self.sidecar_image = v;
        }
        if let Some(v) = get("nas_pvc_name") {
            self.nas_pvc_name = v;
        }
        if let Some(v) = get("webrtc_base_url") {
            self.webrtc_base_url = v;
        }

        if self.jwt_secret.trim().is_empty() {
            return Err(anyhow!(
                "settings 表中未配置 jwt_secret，请在 settings 表写入后再启动"
            ));
        }
        Ok(())
    }
}

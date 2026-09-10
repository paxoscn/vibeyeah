///! 飞书扫码注册应用（device-code flow）
///!
///! 参考：https://github.com/NousResearch/hermes-agent/blob/ad9012097b422ffc968a53748e08c205715aa322/gateway/platforms/feishu.py
///!
///! 流程：
///!   1. POST /oauth/v1/app/registration action=begin → device_code + qr_url
///!   2. 把 qr_url 生成 PNG 图片发给用户
///!   3. 轮询 action=poll 直到用户扫码 → 返回 client_id / client_secret
///!   4. 更新 org_members.lark_app_id / lark_app_secret
use anyhow::{anyhow, Result};
use std::time::{Duration, Instant};

const ACCOUNTS_FEISHU: &str = "https://accounts.feishu.cn";
const REGISTRATION_PATH: &str = "/oauth/v1/app/registration";
const POLL_INTERVAL_DEFAULT: u64 = 5;
const EXPIRE_IN_DEFAULT: u64 = 600;

// ── 注册接口 ──────────────────────────────────────────────────────────────────

pub struct BeginResult {
    pub device_code: String,
    pub qr_url: String,
    pub interval_secs: u64,
    pub expire_in_secs: u64,
}

pub struct PollResult {
    pub app_id: String,
    pub app_secret: String,
    /// 扫码用户的 open_id（app-scoped，来自响应的 user_info.open_id）。
    /// 仅当注册时请求了 user_info 时才存在。
    pub user_open_id: Option<String>,
}

/// 初始化：确认服务端支持 client_secret 认证方式
pub async fn init(base_url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(format!("{base_url}{REGISTRATION_PATH}"))
        .form(&[("action", "init")])
        .send()
        .await?
        .json()
        .await?;

    let methods = resp["supported_auth_methods"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    if !methods.iter().any(|m| m.as_str() == Some("client_secret")) {
        return Err(anyhow!(
            "飞书注册环境不支持 client_secret 认证，当前支持：{:?}",
            methods
        ));
    }
    Ok(())
}

/// 开始注册流程，返回二维码 URL 和 device_code
pub async fn begin(base_url: &str) -> Result<BeginResult> {
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(format!("{base_url}{REGISTRATION_PATH}"))
        .form(&[
            ("action", "begin"),
            ("archetype", "PersonalAgent"),
            ("auth_method", "client_secret"),
            ("request_user_info", "open_id"),
        ])
        .send()
        .await?
        .json()
        .await?;

    let device_code = resp["device_code"]
        .as_str()
        .ok_or_else(|| anyhow!("begin 响应缺少 device_code"))?
        .to_string();

    let qr_url = resp["verification_uri_complete"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let interval_secs = resp["interval"].as_u64().unwrap_or(POLL_INTERVAL_DEFAULT);
    let expire_in_secs = resp["expire_in"].as_u64().unwrap_or(EXPIRE_IN_DEFAULT);

    Ok(BeginResult {
        device_code,
        qr_url,
        interval_secs,
        expire_in_secs,
    })
}

/// 轮询直到用户扫码完成或超时
/// 返回 Ok(Some(result)) 表示成功，Ok(None) 表示超时或被拒绝
pub async fn poll(
    base_url: &str,
    device_code: &str,
    interval_secs: u64,
    expire_in_secs: u64,
) -> Result<Option<PollResult>> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(expire_in_secs);

    loop {
        if Instant::now() >= deadline {
            tracing::warn!("[lark_qr] 轮询超时 ({}s)", expire_in_secs);
            return Ok(None);
        }

        tracing::info!("[lark_qr] 开始 poll");

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;

        let resp: serde_json::Value = match client
            .post(format!("{base_url}{REGISTRATION_PATH}"))
            .form(&[
                ("action", "poll"),
                ("device_code", device_code),
                ("tp", "ob_app"),
            ])
            .send()
            .await
        {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("[lark_qr] 解析 poll 响应失败: {e}");
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("[lark_qr] poll 请求失败: {e}");
                continue;
            }
        };

        // 成功
        if let (Some(app_id), Some(app_secret)) =
            (resp["client_id"].as_str(), resp["client_secret"].as_str())
        {
            let user_open_id = resp["user_info"]["open_id"].as_str().map(str::to_string);
            return Ok(Some(PollResult {
                app_id: app_id.to_string(),
                app_secret: app_secret.to_string(),
                user_open_id,
            }));
        }

        // 终止错误
        let error = resp["error"].as_str().unwrap_or("");
        if matches!(error, "access_denied" | "expired_token") {
            tracing::warn!("[lark_qr] 注册被拒绝: {error}");
            return Ok(None);
        }

        // authorization_pending → 继续轮询
    }
}

// ── 二维码生成（PNG bytes） ───────────────────────────────────────────────────

/// 把 URL 编码成 QR Code，返回 PNG 字节
pub fn qr_to_png(url: &str) -> Result<Vec<u8>> {
    use image::{codecs::png::PngEncoder, ImageEncoder};
    use qrcode::{EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(url, EcLevel::M)
        .map_err(|e| anyhow!("生成 QR Code 失败: {e}"))?;

    // 每个模块渲染为 8×8 像素，安静区 4 模块
    let image = code
        .render::<image::Luma<u8>>()
        .module_dimensions(8, 8)
        .quiet_zone(true)
        .build();

    let mut buf = Vec::new();
    let encoder = PngEncoder::new(&mut buf);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::L8,
        )
        .map_err(|e| anyhow!("编码 PNG 失败: {e}"))?;

    Ok(buf)
}

// ── 上传图片到飞书 ────────────────────────────────────────────────────────────

/// 上传 PNG 字节到飞书图片接口，返回 image_key
pub async fn upload_image_to_lark(token: &str, png_bytes: Vec<u8>) -> Result<String> {
    let part = reqwest::multipart::Part::bytes(png_bytes)
        .file_name("qr.png")
        .mime_str("image/png")?;

    let form = reqwest::multipart::Form::new()
        .text("image_type", "message")
        .part("image", part);

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post("https://open.feishu.cn/open-apis/im/v1/images")
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await?
        .json()
        .await?;

    let code = resp["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(anyhow!(
            "上传图片失败: code={} msg={}",
            code,
            resp["msg"].as_str().unwrap_or("unknown")
        ));
    }

    resp["data"]["image_key"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("响应缺少 image_key"))
}

/// 发送图片消息到飞书
pub async fn send_image_message(
    token: &str,
    receive_id: &str,
    receive_id_type: &str, // "open_id" | "chat_id"
    image_key: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post("https://open.feishu.cn/open-apis/im/v1/messages")
        .query(&[("receive_id_type", receive_id_type)])
        .bearer_auth(token)
        .json(&serde_json::json!({
            "receive_id": receive_id,
            "msg_type": "image",
            "content": serde_json::json!({ "image_key": image_key }).to_string(),
        }))
        .send()
        .await?
        .json()
        .await?;

    let code = resp["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(anyhow!(
            "发送图片消息失败: code={} msg={}",
            code,
            resp["msg"].as_str().unwrap_or("unknown")
        ));
    }

    Ok(resp["data"]["message_id"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

pub fn feishu_accounts_base_url() -> &'static str {
    ACCOUNTS_FEISHU
}

// ── 机器人信息探测 ────────────────────────────────────────────────────────────

/// 机器人基本信息
pub struct BotInfo {
    /// Bot 的 open_id（app-scoped）
    pub open_id: String,
    /// Bot 应用名称
    pub name: String,
}

/// 探测飞书 Bot 信息。
///
/// 参考 hermes-agent probe_bot_http：
///   1. POST /open-apis/auth/v3/tenant_access_token/internal 获取 token
///   2. GET  /open-apis/bot/v3/info 获取 bot.open_id 和 bot.app_name
///
/// 返回 None 表示探测失败（非致命，调用方可静默忽略）。
pub async fn probe_bot(app_id: &str, app_secret: &str) -> Option<BotInfo> {
    let base = "https://open.feishu.cn";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    // 1. 获取 tenant_access_token
    let token_resp: serde_json::Value = client
        .post(format!(
            "{base}/open-apis/auth/v3/tenant_access_token/internal"
        ))
        .json(&serde_json::json!({
            "app_id": app_id,
            "app_secret": app_secret,
        }))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let token = token_resp["tenant_access_token"].as_str()?;

    // 2. 调 /bot/v3/info
    let bot_resp: serde_json::Value = client
        .get(format!("{base}/open-apis/bot/v3/info"))
        .bearer_auth(token)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if bot_resp["code"].as_i64().unwrap_or(-1) != 0 {
        tracing::warn!(
            "[lark_qr] probe_bot 返回错误: code={} msg={}",
            bot_resp["code"],
            bot_resp["msg"].as_str().unwrap_or("unknown")
        );
        return None;
    }

    let bot = &bot_resp["bot"];
    let open_id = bot["open_id"].as_str()?.to_string();
    // bot.v3.info 返回 app_name 字段（和 hermes probe_bot_http 一致）
    let name = bot["app_name"]
        .as_str()
        .or_else(|| bot["bot_name"].as_str()) // 兼容旧版字段名
        .unwrap_or("")
        .to_string();

    tracing::info!(
        "[lark_qr] probe_bot 成功: open_id={} name={}",
        open_id,
        name
    );
    Some(BotInfo { open_id, name })
}

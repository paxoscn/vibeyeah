use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const ILINK_BASE_URL: &str = "https://ilinkai.weixin.qq.com";
const EP_GET_BOT_QR: &str = "ilink/bot/get_bot_qrcode";
const EP_GET_QR_STATUS: &str = "ilink/bot/get_qrcode_status";
const ILINK_APP_ID: &str = "bot";
const ILINK_APP_CLIENT_VERSION: u32 = (2 << 16) | (2 << 8) | 0;
const REQUEST_TIMEOUT_MS: u64 = 15_000;

#[derive(Debug, Deserialize)]
pub struct QrStatusResponse {
    pub status: Option<String>,
    pub redirect_host: Option<String>,
    pub ilink_bot_id: Option<String>,
    pub bot_token: Option<String>,
    pub baseurl: Option<String>,
    pub ilink_user_id: Option<String>,
}

#[derive(Debug)]
pub struct WechatCredentials {
    pub account_id: String,
    pub token: String,
    pub base_url: String,
    pub user_id: String,
}

fn build_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.insert("AuthorizationType", "ilink_bot_token".parse().unwrap());
    headers.insert("iLink-App-Id", ILINK_APP_ID.parse().unwrap());
    headers.insert(
        "iLink-App-ClientVersion",
        ILINK_APP_CLIENT_VERSION.to_string().parse().unwrap(),
    );
    headers
}

pub async fn get_qr_code(bot_type: &str) -> Result<(String, String)> {
    let client = Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()?;

    let url = format!("{ILINK_BASE_URL}/{EP_GET_BOT_QR}?bot_type={bot_type}");

    let resp: serde_json::Value = client
        .get(&url)
        .headers(build_headers())
        .send()
        .await?
        .json()
        .await?;

    let qrcode = resp["qrcode"]
        .as_str()
        .ok_or_else(|| anyhow!("QR 响应缺少 qrcode 字段"))?
        .to_string();

    let qrcode_url = resp["qrcode_img_content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok((qrcode, qrcode_url))
}

pub async fn poll_qr_status(qrcode: &str) -> Result<QrStatusResponse> {
    let client = Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()?;

    let url = format!("{ILINK_BASE_URL}/{EP_GET_QR_STATUS}?qrcode={qrcode}");

    let resp: QrStatusResponse = client
        .get(&url)
        .headers(build_headers())
        .send()
        .await?
        .json()
        .await?;

    Ok(resp)
}

pub fn extract_credentials(status: &QrStatusResponse) -> Option<WechatCredentials> {
    let account_id = status.ilink_bot_id.as_deref().unwrap_or("").to_string();
    let token = status.bot_token.as_deref().unwrap_or("").to_string();
    let base_url = status
        .baseurl
        .as_deref()
        .unwrap_or(ILINK_BASE_URL)
        .to_string();
    let user_id = status.ilink_user_id.as_deref().unwrap_or("").to_string();

    if account_id.is_empty() || token.is_empty() {
        return None;
    }

    Some(WechatCredentials {
        account_id,
        token,
        base_url,
        user_id,
    })
}

use anyhow::{anyhow, Result};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::Config,
    entity::{
        org_member, phone_code,
        user::{self, LoginProvider},
    },
    service::organization,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user id
    pub exp: i64,
}

pub fn generate_jwt(user_id: &Uuid, config: &Config) -> Result<String> {
    let exp = (Utc::now() + chrono::Duration::hours(config.jwt_expire_hours)).timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn verify_jwt(token: &str, config: &Config) -> Result<Claims> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

/// 生成6位随机数字验证码
pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..=999999))
}

/// 发送验证码（此处仅打印，生产中接入短信服务商）
pub async fn send_phone_code(db: &DatabaseConnection, phone: &str) -> Result<()> {
    let code = generate_code();
    let expires_at = Utc::now() + chrono::Duration::minutes(5);

    let model = phone_code::ActiveModel {
        phone: Set(phone.to_string()),
        code: Set(code.clone()),
        expires_at: Set(expires_at.into()),
        used: Set(false),
        ..Default::default()
    };
    model.insert(db).await?;

    // TODO: 接入真实短信服务商
    tracing::info!("【验证码】手机号 {} 验证码: {}", phone, code);
    Ok(())
}

/// 校验验证码并登录/注册用户，返回 JWT
pub async fn login_by_phone(
    db: &DatabaseConnection,
    config: &Config,
    phone: &str,
    code: &str,
) -> Result<String> {
    let now = Utc::now();

    let record = phone_code::Entity::find()
        .filter(phone_code::Column::Phone.eq(phone))
        .filter(phone_code::Column::Code.eq(code))
        .filter(phone_code::Column::Used.eq(false))
        .filter(phone_code::Column::ExpiresAt.gt(now))
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("验证码无效或已过期"))?;

    // 标记已用
    let mut active: phone_code::ActiveModel = record.into();
    active.used = Set(true);
    active.update(db).await?;

    // 查找或创建用户
    let user = user::Entity::find()
        .filter(user::Column::Phone.eq(phone))
        .one(db)
        .await?;

    let user = match user {
        Some(u) => u,
        None => {
            let new_user = user::ActiveModel {
                id: Set(Uuid::new_v4()),
                phone: Set(Some(phone.to_string())),
                provider: Set(LoginProvider::Phone),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };
            let created = new_user.insert(db).await?;
            // 新用户自动加入默认组织
            if let Err(e) = organization::join_default_org(db, created.id).await {
                tracing::warn!("自动加入默认组织失败: {}", e);
            }
            created
        }
    };

    generate_jwt(&user.id, config)
}

/// 飞书 OAuth 回调，用 code 换取用户信息，返回 JWT
pub async fn login_by_lark(
    db: &DatabaseConnection,
    config: &Config,
    lark_code: &str,
) -> Result<String> {
    let lark_user = fetch_lark_user(config, lark_code).await?;
    let now = Utc::now();

    // 通过 org_members.lark_open_id 找到用户（open_id 是 app-scoped，
    // 同一用户在不同飞书应用下 open_id 不同，这里用的是全局 OAuth 应用的 open_id）
    let member = org_member::Entity::find()
        .filter(org_member::Column::LarkOpenId.eq(&lark_user.open_id))
        .one(db)
        .await?;

    let user = if let Some(m) = member {
        user::Entity::find_by_id(m.user_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("关联用户不存在"))?
    } else {
        // 新用户：创建用户记录
        let new_user = user::ActiveModel {
            id: Set(Uuid::new_v4()),
            lark_union_id: Set(lark_user.union_id.clone()),
            nickname: Set(lark_user.name.clone()),
            avatar_url: Set(lark_user.avatar_url.clone()),
            provider: Set(LoginProvider::Lark),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        let created = new_user.insert(db).await?;

        // 自动加入默认组织，并把 open_id 写入成员记录
        if let Err(e) = organization::join_default_org(db, created.id).await {
            tracing::warn!("自动加入默认组织失败: {}", e);
        } else {
            // 更新刚创建的成员记录，写入 lark_open_id
            use crate::service::organization::DEFAULT_ORG_ID;
            let default_id: uuid::Uuid = DEFAULT_ORG_ID.parse().unwrap();
            if let Ok(Some(m)) = org_member::Entity::find()
                .filter(org_member::Column::OrgId.eq(default_id))
                .filter(org_member::Column::UserId.eq(created.id))
                .one(db)
                .await
            {
                let mut active: org_member::ActiveModel = m.into();
                active.lark_open_id = Set(Some(lark_user.open_id.clone()));
                let _ = active.update(db).await;
            }
        }

        created
    };

    generate_jwt(&user.id, config)
}

// ---------- 飞书内部接口 ----------

struct LarkUserInfo {
    open_id: String,
    union_id: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

async fn fetch_lark_user(config: &Config, code: &str) -> Result<LarkUserInfo> {
    let client = reqwest::Client::new();

    // 1. 获取 app_access_token
    let token_resp: serde_json::Value = client
        .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
        .json(&serde_json::json!({
            "app_id": config.lark_app_id,
            "app_secret": config.lark_app_secret,
        }))
        .send()
        .await?
        .json()
        .await?;

    let app_token = token_resp["tenant_access_token"]
        .as_str()
        .ok_or_else(|| anyhow!("飞书 app_access_token 获取失败: {:?}", token_resp))?
        .to_string();

    // 2. 用授权码换用户 access_token
    let user_token_resp: serde_json::Value = client
        .post("https://open.feishu.cn/open-apis/authen/v1/access_token")
        .bearer_auth(&app_token)
        .json(&serde_json::json!({ "grant_type": "authorization_code", "code": code }))
        .send()
        .await?
        .json()
        .await?;

    let data = &user_token_resp["data"];
    let open_id = data["open_id"]
        .as_str()
        .ok_or_else(|| anyhow!("飞书 open_id 缺失: {:?}", user_token_resp))?
        .to_string();

    Ok(LarkUserInfo {
        open_id,
        union_id: data["union_id"].as_str().map(str::to_string),
        name: data["name"].as_str().map(str::to_string),
        avatar_url: data["avatar_url"].as_str().map(str::to_string),
    })
}

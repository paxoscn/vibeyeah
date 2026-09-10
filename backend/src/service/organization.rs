use anyhow::{anyhow, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use uuid::Uuid;

use crate::entity::user::LoginProvider;
use crate::entity::{org_member, organization, user};

/// 首个（默认）组织的固定 UUID。
///
/// 启动向导在库中还没有任何组织时创建，沿用此固定 id。
pub const DEFAULT_ORG_ID: &str = "aaaaaaaa-0000-0000-0000-000000000001";

/// 首个（默认）组织所有者的固定 UUID（启动向导创建，角色 Owner）。
pub const DEFAULT_OWNER_ID: &str = "bbbbbbbb-0000-0000-0000-000000000001";

// ── 启动引导：创建首个组织 ──────────────────────────────────────────────────

pub async fn count_orgs(db: &DatabaseConnection) -> Result<u64> {
    use sea_orm::PaginatorTrait;
    Ok(organization::Entity::find().count(db).await?)
}

/// 获取默认组织（单组织模式）。不存在时返回 None。
pub async fn get_default_org(db: &DatabaseConnection) -> Result<Option<organization::Model>> {
    let default_id: Uuid = DEFAULT_ORG_ID.parse().unwrap();
    Ok(organization::Entity::find_by_id(default_id).one(db).await?)
}

/// 交互式启动引导专用：先创建系统所有者用户，再创建首个组织（固定 `DEFAULT_ORG_ID`），
/// 最后以 Owner 角色把该用户绑定到组织。
pub async fn create_initial_org(
    db: &DatabaseConnection,
    name: &str,
    slug: &str,
    k8s_namespace: &str,
) -> Result<organization::Model> {
    // 1. 先创建所有者用户
    let owner = ensure_initial_owner_user(db).await?;

    // 2. 再创建组织
    let now = Utc::now();
    let org_id: Uuid = DEFAULT_ORG_ID.parse().unwrap();
    let org = organization::ActiveModel {
        id: Set(org_id),
        name: Set(name.to_string()),
        slug: Set(slug.to_string()),
        k8s_namespace: Set(k8s_namespace.to_string()),
        k8s_kubeconfig: Set(None),
        image_pull_secret: Set(None),
        lark_tenant_key: Set(None),
        lark_app_id: Set(None),
        lark_app_secret: Set(None),
        git_url: Set(None),
        git_username: Set(None),
        git_password: Set(None),
        git_ssh_private_key: Set(None),
        openai_base_url: Set(None),
        openai_api_key: Set(None),
        openai_model: Set(None),
        created_by: Set(Some(owner.id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let org = org.insert(db).await?;

    // 3. 最后把所有者用户绑定为组织的 Owner（幂等）
    add_member(db, org_id, owner.id, org_member::OrgRole::Owner).await?;

    Ok(org)
}

/// 创建/复用首个组织的系统所有者用户（幂等，不绑定组织）。
async fn ensure_initial_owner_user(db: &DatabaseConnection) -> Result<user::Model> {
    let owner_id: Uuid = DEFAULT_OWNER_ID.parse().unwrap();
    if let Some(u) = user::Entity::find_by_id(owner_id).one(db).await? {
        return Ok(u);
    }
    let now = Utc::now();
    let u = user::ActiveModel {
        id: Set(owner_id),
        phone: Set(None),
        lark_union_id: Set(None),
        nickname: Set(Some("admin".to_string())),
        avatar_url: Set(None),
        provider: Set(LoginProvider::System),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    Ok(u.insert(db).await?)
}

/// 把启动引导时扫码注册得到的飞书应用凭据写入组织（成为该组织的 Bot 入口）。
pub async fn set_lark_app(
    db: &DatabaseConnection,
    org_id: Uuid,
    app_id: &str,
    app_secret: &str,
) -> Result<()> {
    let org = organization::Entity::find_by_id(org_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("组织不存在: {org_id}"))?;
    let mut active: organization::ActiveModel = org.into();
    active.lark_app_id = Set(Some(app_id.to_string()));
    active.lark_app_secret = Set(Some(app_secret.to_string()));
    active.updated_at = Set(Utc::now().into());
    active.update(db).await?;
    Ok(())
}

/// 组织 Owner 设置组织属性（key 白名单，值为空表示清除）。
pub async fn set_org_property(
    db: &DatabaseConnection,
    org_id: Uuid,
    key: &str,
    value: &str,
) -> Result<()> {
    let org = organization::Entity::find_by_id(org_id)
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("组织不存在"))?;
    let mut active: organization::ActiveModel = org.into();
    let opt = |v: &str| {
        if v.trim().is_empty() {
            None
        } else {
            Some(v.trim().to_string())
        }
    };
    match key {
        "openai_base_url" => active.openai_base_url = Set(opt(value)),
        "openai_api_key" => active.openai_api_key = Set(opt(value)),
        "openai_model" => active.openai_model = Set(opt(value)),
        _ => {
            return Err(anyhow!(
                "不支持的属性：{key}（支持：openai_base_url / openai_api_key / openai_model）"
            ))
        }
    }
    active.updated_at = Set(Utc::now().into());
    active.update(db).await?;
    Ok(())
}

/// 设置组织成员的飞书 open_id（app-scoped）。
pub async fn set_member_lark_open_id(
    db: &DatabaseConnection,
    org_id: Uuid,
    user_id: Uuid,
    open_id: &str,
) -> Result<()> {
    let member = org_member::Entity::find()
        .filter(org_member::Column::OrgId.eq(org_id))
        .filter(org_member::Column::UserId.eq(user_id))
        .one(db)
        .await?
        .ok_or_else(|| anyhow!("成员不存在"))?;
    let mut active: org_member::ActiveModel = member.into();
    active.lark_open_id = Set(Some(open_id.to_string()));
    active.update(db).await?;
    Ok(())
}

// ── 查询 ────────────────────────────────────────────────────────────────────

pub async fn get_member(
    db: &DatabaseConnection,
    org_id: &Uuid,
    user_id: &Uuid,
) -> Result<Option<org_member::Model>> {
    Ok(org_member::Entity::find()
        .filter(org_member::Column::OrgId.eq(*org_id))
        .filter(org_member::Column::UserId.eq(*user_id))
        .one(db)
        .await?)
}

// ── 成员管理 ────────────────────────────────────────────────────────────────

pub async fn add_member(
    db: &DatabaseConnection,
    org_id: Uuid,
    user_id: Uuid,
    role: org_member::OrgRole,
) -> Result<org_member::Model> {
    // 幂等：已存在则直接返回
    if let Some(m) = get_member(db, &org_id, &user_id).await? {
        return Ok(m);
    }
    let now = Utc::now();
    let m = org_member::ActiveModel {
        org_id: Set(org_id),
        user_id: Set(user_id),
        role: Set(role),
        joined_at: Set(now.into()),
        ..Default::default()
    };
    Ok(m.insert(db).await?)
}

/// 新用户注册后自动加入默认组织
pub async fn join_default_org(db: &DatabaseConnection, user_id: Uuid) -> Result<()> {
    let default_id: Uuid = DEFAULT_ORG_ID.parse().unwrap();
    // 如果默认组织不存在（比如测试环境没跑 seed），静默忽略
    if organization::Entity::find_by_id(default_id)
        .one(db)
        .await?
        .is_none()
    {
        tracing::warn!("默认组织不存在，跳过自动加入");
        return Ok(());
    }
    add_member(db, default_id, user_id, org_member::OrgRole::Member).await?;
    Ok(())
}

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum OrgRole {
    /// 创建者，最高权限
    #[sea_orm(string_value = "owner")]
    Owner,
    /// 管理员，可邀请/踢出成员
    #[sea_orm(string_value = "admin")]
    Admin,
    /// 普通成员
    #[sea_orm(string_value = "member")]
    Member,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "org_members")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub role: OrgRole,
    pub joined_at: DateTimeWithTimeZone,
    /// 成员在该组织对应的飞书 open_id（app-scoped，每个飞书应用不同）
    pub lark_open_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organization::Entity",
        from = "Column::OrgId",
        to = "super::organization::Column::Id"
    )]
    Organization,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::organization::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

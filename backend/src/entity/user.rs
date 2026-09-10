use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 登录方式
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum LoginProvider {
    #[sea_orm(string_value = "phone")]
    Phone,
    #[sea_orm(string_value = "lark")]
    Lark,
    /// 系统初始化的所有者（启动向导创建，未绑定手机号/飞书）
    #[sea_orm(string_value = "system")]
    System,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub phone: Option<String>,
    pub lark_union_id: Option<String>,
    pub nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub provider: LoginProvider,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_agent::Entity")]
    UserAgent,
    #[sea_orm(has_many = "super::conversation::Entity")]
    Conversation,
    #[sea_orm(has_many = "super::org_member::Entity")]
    OrgMember,
}

impl Related<super::user_agent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserAgent.def()
    }
}

impl Related<super::conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Conversation.def()
    }
}

impl Related<super::org_member::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrgMember.def()
    }
}

// 通过 user_agents 关联到 agents
impl Related<super::agent::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_agent::Relation::Agent.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::user_agent::Relation::User.def().rev())
    }
}

// 通过 org_members 关联到 organizations
impl Related<super::organization::Entity> for Entity {
    fn to() -> RelationDef {
        super::org_member::Relation::Organization.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::org_member::Relation::User.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

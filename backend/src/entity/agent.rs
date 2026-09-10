use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::user_agent;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum PodStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "creating")]
    Creating,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "succeeded")]
    Succeeded,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub system_prompt: Option<String>,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,

    /// 所属组织（多对一）
    pub org_id: Option<Uuid>,

    // K8s 容器信息
    pub pod_name: Option<String>,
    pub pod_namespace: Option<String>,
    pub pod_status: PodStatus,
    pub pod_synced_at: Option<DateTimeWithTimeZone>,

    // WebRTC 推流
    pub webrtc_url: Option<String>,
    pub stream_active: bool,

    // 飞书 Bot 配置（用于注入到容器环境变量）
    pub lark_app_id: Option<String>,
    pub lark_app_secret: Option<String>,
    pub lark_bot_open_id: Option<String>,
    pub lark_bot_name: Option<String>,

    // 微信配置
    pub wechat_account_id: Option<String>,
    pub wechat_token: Option<String>,
    pub wechat_base_url: Option<String>,
    pub wechat_user_id: Option<String>,

    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::conversation::Entity")]
    Conversation,
    #[sea_orm(has_many = "super::user_agent::Entity")]
    UserAgent,
    #[sea_orm(
        belongs_to = "super::organization::Entity",
        from = "Column::OrgId",
        to = "super::organization::Column::Id"
    )]
    Organization,
}

impl Related<super::conversation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Conversation.def()
    }
}

impl Related<super::user_agent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserAgent.def()
    }
}

impl Related<super::organization::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        user_agent::Relation::User.def()
    }

    fn via() -> Option<RelationDef> {
        Some(user_agent::Relation::Agent.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

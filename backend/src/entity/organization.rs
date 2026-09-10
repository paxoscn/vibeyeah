use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "organizations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// 显示名称
    pub name: String,
    /// URL 友好唯一标识，全小写，如 "vibeyeah"
    #[sea_orm(unique)]
    pub slug: String,
    /// 该组织在 K8s 上使用的 namespace
    pub k8s_namespace: String,
    /// 可选：组织专属 kubeconfig（base64 encoded），为 None 时使用全局默认
    pub k8s_kubeconfig: Option<String>,
    /// 可选：拉取镜像用的 K8s Secret 名称（imagePullSecrets）
    pub image_pull_secret: Option<String>,
    /// 可选：飞书组织标识（tenant_key），用于关联飞书企业
    pub lark_tenant_key: Option<String>,
    /// 可选：飞书应用 App ID（每个组织可以有独立的飞书应用）
    pub lark_app_id: Option<String>,
    /// 可选：飞书应用 App Secret
    pub lark_app_secret: Option<String>,
    /// 可选：agent 状态同步 git 仓库地址
    pub git_url: Option<String>,
    /// 可选：git https 认证用户名
    pub git_username: Option<String>,
    /// 可选：git https 认证密码或 PAT
    pub git_password: Option<String>,
    /// 可选：git SSH 私钥（PEM 格式）
    pub git_ssh_private_key: Option<String>,
    /// 可选：组织级 OpenAI 兼容 LLM Base URL（平台 bot 对话使用）
    pub openai_base_url: Option<String>,
    /// 可选：组织级 OpenAI 兼容 LLM API Key
    pub openai_api_key: Option<String>,
    /// 可选：组织级 OpenAI 兼容 LLM 模型名
    pub openai_model: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::org_member::Entity")]
    OrgMember,
    #[sea_orm(has_many = "super::agent::Entity")]
    Agent,
}

impl Related<super::org_member::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrgMember.def()
    }
}

impl Related<super::agent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Agent.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        super::org_member::Relation::User.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::org_member::Relation::Organization.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

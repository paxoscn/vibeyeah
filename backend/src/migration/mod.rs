use sea_orm_migration::prelude::*;

mod m20240001_create_users;
mod m20240002_create_phone_codes;
mod m20240003_create_agents;
mod m20240004_create_user_agents_and_conversations;
mod m20240005_agents_add_pod_fields;
mod m20240006_agents_add_webrtc_fields;
mod m20240007_create_organizations;
mod m20240009_agents_add_org_id;
mod m20240010_orgs_add_lark_tenant_key;
mod m20240011_orgs_add_lark_app;
mod m20240012_org_members_add_lark_app;
mod m20240013_move_lark_open_id_to_org_members;
mod m20240014_org_members_add_lark_bot_info;
mod m20240015_agents_add_lark_bot_fields;
mod m20240016_orgs_add_image_pull_secret;
mod m20240017_agents_add_wechat_fields;
mod m20240018_orgs_add_git_config;
mod m20240019_create_settings;
mod m20240020_orgs_add_openai_fields;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240001_create_users::Migration),
            Box::new(m20240002_create_phone_codes::Migration),
            Box::new(m20240003_create_agents::Migration),
            Box::new(m20240004_create_user_agents_and_conversations::Migration),
            Box::new(m20240005_agents_add_pod_fields::Migration),
            Box::new(m20240006_agents_add_webrtc_fields::Migration),
            Box::new(m20240007_create_organizations::Migration),
            Box::new(m20240009_agents_add_org_id::Migration),
            Box::new(m20240010_orgs_add_lark_tenant_key::Migration),
            Box::new(m20240011_orgs_add_lark_app::Migration),
            Box::new(m20240012_org_members_add_lark_app::Migration),
            Box::new(m20240013_move_lark_open_id_to_org_members::Migration),
            Box::new(m20240014_org_members_add_lark_bot_info::Migration),
            Box::new(m20240015_agents_add_lark_bot_fields::Migration),
            Box::new(m20240016_orgs_add_image_pull_secret::Migration),
            Box::new(m20240017_agents_add_wechat_fields::Migration),
            Box::new(m20240018_orgs_add_git_config::Migration),
            Box::new(m20240019_create_settings::Migration),
            Box::new(m20240020_orgs_add_openai_fields::Migration),
        ]
    }
}

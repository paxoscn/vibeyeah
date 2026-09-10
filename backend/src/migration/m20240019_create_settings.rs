use sea_orm_migration::prelude::*;

/// 平台配置表：key/value。启动时后端一次性读入 `Config`。
/// 默认值与旧的环境变量默认一致；密钥类（jwt_secret / openai_api_key /
/// lark_app_secret / sms_* / callback_token）默认留空，部署时写入真实值。
#[derive(DeriveMigrationName)]
pub struct Migration;

const DEFAULT_SETTINGS: &[(&str, &str)] = &[
    ("jwt_expire_hours", "72"),
    ("jwt_secret", "change_me_to_a_long_random_secret"),
    ("lark_app_id", ""),
    ("lark_app_secret", ""),
    ("k8s_namespace", "default"),
    ("pod_sync_interval_secs", "30"),
    ("nas_mount_root", "/data/nas"),
    ("hermes_exec_timeout_secs", "900"),
    ("callback_token", ""),
    ("desktop_image", "vibeyeah/desktop:latest"),
    ("sidecar_image", "vibeyeah/sidecar:latest"),
    ("nas_pvc_name", "vibeyeah-nas-pvc"),
    ("webrtc_base_url", "http://localhost:8889"),
    ("sms_access_key_id", ""),
    ("sms_access_key_secret", ""),
    ("sms_sign_name", ""),
    ("sms_template_code", ""),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Settings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Settings::Key)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Settings::Value).string().not_null())
                    .to_owned(),
            )
            .await?;

        // 种子默认值（INSERT ... SELECT ... WHERE NOT EXISTS，兼容 PostgreSQL 与 SQLite；均为常量，无注入风险）
        let conn = manager.get_connection();
        for (k, v) in DEFAULT_SETTINGS {
            conn.execute_unprepared(&format!(
                "INSERT INTO settings (key, value) SELECT '{}', '{}' WHERE NOT EXISTS (SELECT 1 FROM settings WHERE key = '{}')",
                k, v, k
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Settings::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Settings {
    Table,
    Key,
    Value,
}

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240017_agents_add_wechat_fields"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 每次 ALTER TABLE 仅支持一列操作，故逐列执行
        for col in [
            ColumnDef::new(Agents::WechatAccountId).string_len(128),
            ColumnDef::new(Agents::WechatToken).string_len(512),
            ColumnDef::new(Agents::WechatBaseUrl).string_len(512),
            ColumnDef::new(Agents::WechatUserId).string_len(128),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Agents::Table)
                        .add_column_if_not_exists(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for col in [
            Agents::WechatAccountId,
            Agents::WechatToken,
            Agents::WechatBaseUrl,
            Agents::WechatUserId,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Agents::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(Iden)]
enum Agents {
    Table,
    WechatAccountId,
    WechatToken,
    WechatBaseUrl,
    WechatUserId,
}

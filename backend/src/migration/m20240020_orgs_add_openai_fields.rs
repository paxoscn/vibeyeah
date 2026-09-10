use sea_orm_migration::prelude::*;

/// 组织级 OpenAI 兼容 LLM 配置（默认未配置，空值）。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 每次 ALTER TABLE 仅支持一列操作，故逐列执行
        for col in [
            ColumnDef::new(Organizations::OpenaiBaseUrl).string().null(),
            ColumnDef::new(Organizations::OpenaiApiKey).string().null(),
            ColumnDef::new(Organizations::OpenaiModel).string().null(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Organizations::Table)
                        .add_column_if_not_exists(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for col in [
            Organizations::OpenaiBaseUrl,
            Organizations::OpenaiApiKey,
            Organizations::OpenaiModel,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Organizations::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(Iden)]
enum Organizations {
    Table,
    OpenaiBaseUrl,
    OpenaiApiKey,
    OpenaiModel,
}

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240011_orgs_add_lark_app"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 每次 ALTER TABLE 仅支持一列操作，故逐列执行
        manager
            .alter_table(
                Table::alter()
                    .table(Organizations::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Organizations::LarkAppId).string_len(64),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Organizations::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Organizations::LarkAppSecret).string_len(128),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Organizations::Table)
                    .drop_column(Organizations::LarkAppId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Organizations::Table)
                    .drop_column(Organizations::LarkAppSecret)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Organizations {
    Table,
    LarkAppId,
    LarkAppSecret,
}

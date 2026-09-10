use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240006_agents_add_webrtc_fields"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 每次 ALTER TABLE 仅支持一列操作，故逐列执行
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column_if_not_exists(ColumnDef::new(Agents::WebrtcUrl).text())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Agents::StreamActive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .drop_column(Agents::WebrtcUrl)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .drop_column(Agents::StreamActive)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Agents {
    Table,
    WebrtcUrl,
    StreamActive,
}

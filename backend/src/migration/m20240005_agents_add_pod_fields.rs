use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240005_agents_add_pod_fields"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 每次 ALTER TABLE 只允许一列，故逐列执行（PostgreSQL 同样适用）
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column_if_not_exists(ColumnDef::new(Agents::PodName).string_len(128))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column_if_not_exists(ColumnDef::new(Agents::PodNamespace).string_len(64))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Agents::PodStatus)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Agents::PodSyncedAt).timestamp_with_time_zone(),
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
                    .drop_column(Agents::PodName)
                    .drop_column(Agents::PodNamespace)
                    .drop_column(Agents::PodStatus)
                    .drop_column(Agents::PodSyncedAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Agents {
    Table,
    PodName,
    PodNamespace,
    PodStatus,
    PodSyncedAt,
}

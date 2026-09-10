use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240009_agents_add_org_id"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column_if_not_exists(ColumnDef::new(Agents::OrgId).uuid())
                    .to_owned(),
            )
            .await?;

        // SQLite 不支持对已有表修改外键约束，跳过（SQLite 默认不强制外键）
        if manager.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_agents_org_id")
                        .from(Agents::Table, Agents::OrgId)
                        .to(Organizations::Table, Organizations::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_agents_org_id")
                    .table(Agents::Table)
                    .col(Agents::OrgId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .table(Agents::Table)
                        .name("fk_agents_org_id")
                        .to_owned(),
                )
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .drop_column(Agents::OrgId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Agents {
    Table,
    OrgId,
}

#[derive(Iden)]
enum Organizations {
    Table,
    Id,
}

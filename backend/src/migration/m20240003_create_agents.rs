use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240003_create_agents"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Agents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Agents::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Agents::Name).string_len(128).not_null())
                    .col(ColumnDef::new(Agents::SystemPrompt).text())
                    .col(ColumnDef::new(Agents::Description).text())
                    .col(ColumnDef::new(Agents::CreatedBy).uuid())
                    .col(
                        ColumnDef::new(Agents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Agents::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Agents::Table, Agents::CreatedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Agents::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Agents {
    Table,
    Id,
    Name,
    SystemPrompt,
    Description,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240001_create_users"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Users::Phone).string_len(20).unique_key())
                    .col(
                        ColumnDef::new(Users::LarkOpenId)
                            .string_len(64)
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::LarkUnionId).string_len(64))
                    .col(ColumnDef::new(Users::Nickname).string_len(64))
                    .col(ColumnDef::new(Users::AvatarUrl).text())
                    .col(
                        ColumnDef::new(Users::Provider)
                            .string_len(20)
                            .not_null()
                            .default("phone"),
                    )
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Users::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Phone,
    LarkOpenId,
    LarkUnionId,
    Nickname,
    AvatarUrl,
    Provider,
    CreatedAt,
    UpdatedAt,
}

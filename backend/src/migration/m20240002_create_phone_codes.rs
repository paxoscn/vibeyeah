use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240002_create_phone_codes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PhoneCodes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PhoneCodes::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PhoneCodes::Phone).string_len(20).not_null())
                    .col(ColumnDef::new(PhoneCodes::Code).string_len(8).not_null())
                    .col(
                        ColumnDef::new(PhoneCodes::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PhoneCodes::Used)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(PhoneCodes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_phone_codes_phone_used_expires")
                    .table(PhoneCodes::Table)
                    .col(PhoneCodes::Phone)
                    .col(PhoneCodes::Used)
                    .col(PhoneCodes::ExpiresAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PhoneCodes::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PhoneCodes {
    Table,
    Id,
    Phone,
    Code,
    ExpiresAt,
    Used,
    CreatedAt,
}

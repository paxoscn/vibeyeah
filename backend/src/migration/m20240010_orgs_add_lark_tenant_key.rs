use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240010_orgs_add_lark_tenant_key"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Organizations::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Organizations::LarkTenantKey).string_len(64),
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
                    .drop_column(Organizations::LarkTenantKey)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Organizations {
    Table,
    LarkTenantKey,
}

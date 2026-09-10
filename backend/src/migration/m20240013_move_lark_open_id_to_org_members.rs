use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240013_move_lark_open_id_to_org_members"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. org_members 加 lark_open_id
        manager
            .alter_table(
                Table::alter()
                    .table(OrgMembers::Table)
                    .add_column_if_not_exists(ColumnDef::new(OrgMembers::LarkOpenId).string_len(64))
                    .to_owned(),
            )
            .await?;

        // 2. users 删除 lark_open_id（数据已无法自动迁移到 org_members，
        //    因为一个用户可能属于多个组织，open_id 是 app-scoped 的）。
        //    SQLite 不允许删除 UNIQUE 列，故跳过（该列在 SQLite 中保留但不被代码使用）。
        if manager.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(Users::Table)
                        .drop_column(Users::LarkOpenId)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrgMembers::Table)
                    .drop_column(OrgMembers::LarkOpenId)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(Users::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(Users::LarkOpenId)
                                .string_len(64)
                                .unique_key(),
                        )
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(Iden)]
enum OrgMembers {
    Table,
    LarkOpenId,
}

#[derive(Iden)]
enum Users {
    Table,
    LarkOpenId,
}

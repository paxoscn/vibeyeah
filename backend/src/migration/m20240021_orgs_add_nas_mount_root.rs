use sea_orm_migration::prelude::*;

/// 组织级 NAS 挂载根目录（创建组织时按后端 cwd 生成，NULL 表示回退默认 `/data/nas`）。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Organizations::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Organizations::NasMountRoot).string().null(),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Organizations::Table)
                    .drop_column(Organizations::NasMountRoot)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Organizations {
    Table,
    NasMountRoot,
}

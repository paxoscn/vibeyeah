use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 每次 ALTER TABLE 仅支持一列操作，故逐列执行
        for col in [
            ColumnDef::new(Organizations::GitUrl).string().null(),
            ColumnDef::new(Organizations::GitUsername).string().null(),
            ColumnDef::new(Organizations::GitPassword).string().null(),
            ColumnDef::new(Organizations::GitSshPrivateKey)
                .text()
                .null(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Organizations::Table)
                        .add_column_if_not_exists(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for col in [
            Organizations::GitUrl,
            Organizations::GitUsername,
            Organizations::GitPassword,
            Organizations::GitSshPrivateKey,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Organizations::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(Iden)]
enum Organizations {
    Table,
    GitUrl,
    GitUsername,
    GitPassword,
    GitSshPrivateKey,
}

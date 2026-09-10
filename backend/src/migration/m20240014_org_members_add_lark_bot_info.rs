use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240014_org_members_add_lark_bot_info"
    }
}

// 原本错误地把这些字段加到了 org_members，此迁移保持原样以兼容已跑过的数据库。
// 实际正确的位置是 agents 表，由后续迁移 m20240015 完成迁移。
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 每次 ALTER TABLE 仅支持一列操作，故逐列执行
        manager
            .alter_table(
                Table::alter()
                    .table(OrgMembers::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(OrgMembers::LarkBotOpenId).string_len(64),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(OrgMembers::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(OrgMembers::LarkBotName).string_len(128),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrgMembers::Table)
                    .drop_column(OrgMembers::LarkBotOpenId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(OrgMembers::Table)
                    .drop_column(OrgMembers::LarkBotName)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum OrgMembers {
    Table,
    LarkBotOpenId,
    LarkBotName,
}

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240015_agents_add_lark_bot_fields"
    }
}

/// 把飞书 Bot 配置字段从 org_members 迁到正确的位置：agents 表。
/// 同时从 org_members 删除错误加入的列。
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. agents 表加 4 个字段（SQLite 每次 ALTER TABLE 仅支持一列，故逐列）
        for col in [
            ColumnDef::new(Agents::LarkAppId).string_len(64),
            ColumnDef::new(Agents::LarkAppSecret).string_len(128),
            ColumnDef::new(Agents::LarkBotOpenId).string_len(64),
            ColumnDef::new(Agents::LarkBotName).string_len(128),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Agents::Table)
                        .add_column_if_not_exists(col)
                        .to_owned(),
                )
                .await?;
        }

        // 2. org_members 删除错误列（已由 m20240012 / m20240014 加入）
        for col in [
            OrgMembers::LarkAppId,
            OrgMembers::LarkAppSecret,
            OrgMembers::LarkBotOpenId,
            OrgMembers::LarkBotName,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(OrgMembers::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚：从 agents 删除，恢复到 org_members
        for col in [
            Agents::LarkAppId,
            Agents::LarkAppSecret,
            Agents::LarkBotOpenId,
            Agents::LarkBotName,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Agents::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }

        for col in [
            ColumnDef::new(OrgMembers::LarkAppId).string_len(64),
            ColumnDef::new(OrgMembers::LarkAppSecret).string_len(128),
            ColumnDef::new(OrgMembers::LarkBotOpenId).string_len(64),
            ColumnDef::new(OrgMembers::LarkBotName).string_len(128),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(OrgMembers::Table)
                        .add_column_if_not_exists(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(Iden)]
enum Agents {
    Table,
    LarkAppId,
    LarkAppSecret,
    LarkBotOpenId,
    LarkBotName,
}

#[derive(Iden)]
enum OrgMembers {
    Table,
    LarkAppId,
    LarkAppSecret,
    LarkBotOpenId,
    LarkBotName,
}

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240004_create_user_agents_and_conversations"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // user_agents：用户-智能体分配关系表
        manager
            .create_table(
                Table::create()
                    .table(UserAgents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserAgents::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserAgents::UserId).uuid().not_null())
                    .col(ColumnDef::new(UserAgents::AgentId).uuid().not_null())
                    .col(
                        ColumnDef::new(UserAgents::AssignedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserAgents::Table, UserAgents::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserAgents::Table, UserAgents::AgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 唯一约束：同一用户同一智能体只能分配一次
        manager
            .create_index(
                Index::create()
                    .name("idx_user_agents_user_agent")
                    .unique()
                    .table(UserAgents::Table)
                    .col(UserAgents::UserId)
                    .col(UserAgents::AgentId)
                    .to_owned(),
            )
            .await?;

        // conversations：会话表
        manager
            .create_table(
                Table::create()
                    .table(Conversations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Conversations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Conversations::AgentId).uuid().not_null())
                    .col(ColumnDef::new(Conversations::UserId).uuid().not_null())
                    .col(ColumnDef::new(Conversations::Title).string_len(256))
                    .col(
                        ColumnDef::new(Conversations::Status)
                            .string_len(20)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Conversations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Conversations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Conversations::Table, Conversations::AgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Conversations::Table, Conversations::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_conversations_agent_user")
                    .table(Conversations::Table)
                    .col(Conversations::AgentId)
                    .col(Conversations::UserId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Conversations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserAgents::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum UserAgents {
    Table,
    Id,
    UserId,
    AgentId,
    AssignedAt,
}

#[derive(Iden)]
enum Conversations {
    Table,
    Id,
    AgentId,
    UserId,
    Title,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

#[derive(Iden)]
enum Agents {
    Table,
    Id,
}

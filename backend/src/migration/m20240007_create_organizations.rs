use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240007_create_organizations"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // organizations
        manager
            .create_table(
                Table::create()
                    .table(Organizations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Organizations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Organizations::Name)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Organizations::Slug)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Organizations::K8sNamespace)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Organizations::K8sKubeconfig).text())
                    .col(ColumnDef::new(Organizations::CreatedBy).uuid())
                    .col(
                        ColumnDef::new(Organizations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Organizations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Organizations::Table, Organizations::CreatedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // org_members
        manager
            .create_table(
                Table::create()
                    .table(OrgMembers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrgMembers::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrgMembers::OrgId).uuid().not_null())
                    .col(ColumnDef::new(OrgMembers::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(OrgMembers::Role)
                            .string_len(20)
                            .not_null()
                            .default("member"),
                    )
                    .col(
                        ColumnDef::new(OrgMembers::JoinedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrgMembers::Table, OrgMembers::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrgMembers::Table, OrgMembers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 唯一约束：一个用户在同一组织只能有一条成员记录
        manager
            .create_index(
                Index::create()
                    .name("idx_org_members_org_user")
                    .unique()
                    .table(OrgMembers::Table)
                    .col(OrgMembers::OrgId)
                    .col(OrgMembers::UserId)
                    .to_owned(),
            )
            .await?;

        // org_invitations
        manager
            .create_table(
                Table::create()
                    .table(OrgInvitations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrgInvitations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrgInvitations::OrgId).uuid().not_null())
                    .col(
                        ColumnDef::new(OrgInvitations::Token)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(OrgInvitations::InviteePhone).string_len(20))
                    .col(ColumnDef::new(OrgInvitations::InvitedBy).uuid().not_null())
                    .col(
                        ColumnDef::new(OrgInvitations::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrgInvitations::Used)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(OrgInvitations::UsedBy).uuid())
                    .col(
                        ColumnDef::new(OrgInvitations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrgInvitations::Table, OrgInvitations::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrgInvitations::Table, OrgInvitations::InvitedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrgInvitations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OrgMembers::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Organizations::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Organizations {
    Table,
    Id,
    Name,
    Slug,
    K8sNamespace,
    K8sKubeconfig,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum OrgMembers {
    Table,
    Id,
    OrgId,
    UserId,
    Role,
    JoinedAt,
}

#[derive(Iden)]
enum OrgInvitations {
    Table,
    Id,
    OrgId,
    Token,
    InviteePhone,
    InvitedBy,
    ExpiresAt,
    Used,
    UsedBy,
    CreatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

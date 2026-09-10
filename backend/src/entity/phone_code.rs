use sea_orm::entity::prelude::*;

/// 手机验证码（短期有效，验证后即可删除）
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "phone_codes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub phone: String,
    pub code: String,
    pub expires_at: DateTimeWithTimeZone,
    pub used: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

/// 平台级配置项（key/value）。后端启动时一次性加载进 `Config`，
/// 除 `DATABASE_URL` / `BIND_ADDR`（仍来自环境变量）外的运行配置均存于此表。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    pub value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "transactions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub signature: String,
    pub slot: i64,
    pub block_time: Option<DateTime<Utc>>,
    pub fee: Option<i64>,
    pub success: bool,
    #[sea_orm(column_name = "err")]
    pub error: Option<Json>,
    pub num_instructions: Option<i32>,
    pub accounts: Vec<String>,
    pub log_messages: Vec<String>,
    pub has_cpi_data: bool,
    pub source: String,
    pub raw: Option<Json>,
    pub indexed_at: DateTime<Utc>,
    pub enriched_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::inner_instruction::Entity")]
    InnerInstructions,
    #[sea_orm(has_many = "super::account_touched::Entity")]
    AccountsTouched,
}

impl Related<super::inner_instruction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InnerInstructions.def()
    }
}

impl Related<super::account_touched::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccountsTouched.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

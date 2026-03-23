use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inner_instructions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub signature: String,
    pub instruction_idx: i32,
    pub depth: i32,
    pub program_id: String,
    pub accounts: Vec<String>,
    pub data: Option<String>,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::transaction::Entity",
        from = "Column::Signature",
        to = "super::transaction::Column::Signature"
    )]
    Transaction,
}

impl Related<super::transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Transaction.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

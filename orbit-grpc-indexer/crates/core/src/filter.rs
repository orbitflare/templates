use crate::types::RawTransaction;

pub trait TransactionFilter: Send + Sync {
    fn filter(&self, tx: &RawTransaction) -> bool;
    fn name(&self) -> &str;
}

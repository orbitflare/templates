use std::collections::HashSet;

use indexer_core::filter::TransactionFilter;
use indexer_core::types::RawTransaction;

pub struct ProgramFilter {
    programs: HashSet<String>,
}

impl ProgramFilter {
    pub fn new(program_ids: Vec<String>) -> Self {
        Self {
            programs: program_ids.into_iter().collect(),
        }
    }
}

impl TransactionFilter for ProgramFilter {
    fn filter(&self, tx: &RawTransaction) -> bool {
        if self.programs.is_empty() {
            return true;
        }
        tx.account_keys.iter().any(|key| self.programs.contains(key))
    }

    fn name(&self) -> &str {
        "program"
    }
}

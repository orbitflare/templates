use std::collections::HashSet;

use indexer_core::filter::TransactionFilter;
use indexer_core::types::RawTransaction;

pub struct AccountFilter {
    accounts: HashSet<String>,
}

impl AccountFilter {
    pub fn new(accounts: Vec<String>) -> Self {
        Self {
            accounts: accounts.into_iter().collect(),
        }
    }
}

impl TransactionFilter for AccountFilter {
    fn filter(&self, tx: &RawTransaction) -> bool {
        if self.accounts.is_empty() {
            return true;
        }
        tx.account_keys.iter().any(|key| self.accounts.contains(key))
    }

    fn name(&self) -> &str {
        "account"
    }
}

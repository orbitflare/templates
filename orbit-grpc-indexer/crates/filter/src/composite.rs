use indexer_core::filter::TransactionFilter;
use indexer_core::types::RawTransaction;

pub struct CompositeFilter {
    filters: Vec<Box<dyn TransactionFilter>>,
}

impl CompositeFilter {
    pub fn new(filters: Vec<Box<dyn TransactionFilter>>) -> Self {
        Self { filters }
    }

    pub fn add(&mut self, filter: Box<dyn TransactionFilter>) {
        self.filters.push(filter);
    }

    pub fn from_config(config: &indexer_config::model::FilterConfig) -> Self {
        let mut filters: Vec<Box<dyn TransactionFilter>> = Vec::new();

        if !config.program_ids.is_empty() {
            filters.push(Box::new(
                crate::program::ProgramFilter::new(config.program_ids.clone()),
            ));
        }

        if !config.accounts.is_empty() {
            filters.push(Box::new(
                crate::account::AccountFilter::new(config.accounts.clone()),
            ));
        }

        if config.require_success {
            filters.push(Box::new(SuccessFilter));
        }

        Self::new(filters)
    }
}

impl TransactionFilter for CompositeFilter {
    fn filter(&self, tx: &RawTransaction) -> bool {
        self.filters.iter().all(|f| f.filter(tx))
    }

    fn name(&self) -> &str {
        "composite"
    }
}

struct SuccessFilter;

impl TransactionFilter for SuccessFilter {
    fn filter(&self, tx: &RawTransaction) -> bool {
        tx.success
    }

    fn name(&self) -> &str {
        "success"
    }
}

use crate::config::AppConfig;
use jetstream_protos::jetstream::SubscribeRequestFilterTransactions;
use std::collections::HashMap;

pub fn build_subscription_filters(
    config: &AppConfig,
) -> HashMap<String, SubscribeRequestFilterTransactions> {
    let mut filters = HashMap::new();

    let enabled_targets: Vec<&str> = config
        .targets
        .iter()
        .filter(|t| t.enabled)
        .map(|t| t.address.as_str())
        .collect();

    if enabled_targets.is_empty() {
        tracing::warn!("No enabled target wallets configured");
        return filters;
    }

    tracing::info!(
        "Building subscription filter for {} target wallet(s)",
        enabled_targets.len()
    );

    let filter = SubscribeRequestFilterTransactions {
        account_include: enabled_targets.iter().map(|a| a.to_string()).collect(),
        account_exclude: vec![],
        account_required: vec![],
    };

    filters.insert("copy_targets".to_string(), filter);
    filters
}

use crate::config::AppConfig;
use orbitflare_sdk::grpc::TransactionFilter as GeyserTransactionFilter;
use orbitflare_sdk::jetstream::v2::TransactionFilter;
use orbitflare_sdk::proto::geyser::SubscribeRequestFilterTransactions;
use orbitflare_sdk::proto::jetstream::v2::TxFilter;
use std::collections::HashMap;

pub fn build_jetstream_filters(config: &AppConfig) -> Vec<TxFilter> {
    let enabled_targets: Vec<String> = config
        .targets
        .iter()
        .filter(|t| t.enabled)
        .map(|t| t.address.clone())
        .collect();

    if enabled_targets.is_empty() {
        tracing::warn!("No enabled target wallets configured");
        return Vec::new();
    }

    tracing::info!(
        "Building Jetstream filter for {} target wallet(s)",
        enabled_targets.len()
    );

    vec![
        TransactionFilter::new()
            .account_include(enabled_targets)
            .with_id("copy_targets"),
    ]
}

pub fn build_yellowstone_filters(
    config: &AppConfig,
) -> HashMap<String, SubscribeRequestFilterTransactions> {
    let mut filters = HashMap::new();

    let enabled_targets: Vec<String> = config
        .targets
        .iter()
        .filter(|t| t.enabled)
        .map(|t| t.address.clone())
        .collect();

    if enabled_targets.is_empty() {
        tracing::warn!("No enabled target wallets configured for Yellowstone");
        return filters;
    }

    tracing::info!(
        "Building Yellowstone filter for {} target wallet(s)",
        enabled_targets.len()
    );

    let filter = GeyserTransactionFilter::new()
        .vote(false)
        .failed(false)
        .account_include(enabled_targets)
        .into();

    filters.insert("copy_targets".to_string(), filter);
    filters
}

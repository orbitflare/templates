use crate::config::AppConfig;
use std::collections::HashMap;

pub fn build_jetstream_filters(
    config: &AppConfig,
) -> HashMap<String, jetstream_protos::jetstream::SubscribeRequestFilterTransactions> {
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
        "Building Jetstream filter for {} target wallet(s)",
        enabled_targets.len()
    );

    let filter = jetstream_protos::jetstream::SubscribeRequestFilterTransactions {
        account_include: enabled_targets.iter().map(|a| a.to_string()).collect(),
        account_exclude: vec![],
        account_required: vec![],
    };

    filters.insert("copy_targets".to_string(), filter);
    filters
}

pub fn build_yellowstone_filters(
    config: &AppConfig,
) -> HashMap<String, yellowstone_protos::geyser::SubscribeRequestFilterTransactions> {
    let mut filters = HashMap::new();

    let enabled_targets: Vec<&str> = config
        .targets
        .iter()
        .filter(|t| t.enabled)
        .map(|t| t.address.as_str())
        .collect();

    if enabled_targets.is_empty() {
        tracing::warn!("No enabled target wallets configured for Yellowstone");
        return filters;
    }

    tracing::info!(
        "Building Yellowstone filter for {} target wallet(s)",
        enabled_targets.len()
    );

    let filter = yellowstone_protos::geyser::SubscribeRequestFilterTransactions {
        vote: Some(false),
        failed: Some(false),
        signature: None,
        account_include: enabled_targets.iter().map(|a| a.to_string()).collect(),
        account_exclude: vec![],
        account_required: vec![],
    };

    filters.insert("copy_targets".to_string(), filter);
    filters
}

use std::sync::atomic::{AtomicU64, Ordering};

pub struct Metrics {
    pub jetstream_received: AtomicU64,
    pub yellowstone_received: AtomicU64,
    pub total_indexed: AtomicU64,
    pub total_filtered: AtomicU64,
    pub total_errors: AtomicU64,
    pub jetstream_connected: AtomicU64,
    pub yellowstone_connected: AtomicU64,
}

pub struct MetricsSnapshot {
    pub jetstream_received: u64,
    pub yellowstone_received: u64,
    pub total_indexed: u64,
    pub total_filtered: u64,
    pub total_errors: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            jetstream_received: AtomicU64::new(0),
            yellowstone_received: AtomicU64::new(0),
            total_indexed: AtomicU64::new(0),
            total_filtered: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            jetstream_connected: AtomicU64::new(0),
            yellowstone_connected: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            jetstream_received: self.jetstream_received.load(Ordering::Relaxed),
            yellowstone_received: self.yellowstone_received.load(Ordering::Relaxed),
            total_indexed: self.total_indexed.load(Ordering::Relaxed),
            total_filtered: self.total_filtered.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
        }
    }
}

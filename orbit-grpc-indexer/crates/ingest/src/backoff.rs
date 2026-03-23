use std::time::Duration;

use indexer_config::model::ReconnectConfig;

pub struct Backoff {
    base_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
    max_retries: u64,
    attempt: u64,
}

impl Backoff {
    pub fn from_config(config: &ReconnectConfig) -> Self {
        Self {
            base_delay: Duration::from_millis(config.base_delay_ms),
            max_delay: Duration::from_millis(config.max_delay_ms),
            multiplier: config.multiplier,
            max_retries: config.max_retries,
            attempt: 0,
        }
    }

    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.max_retries > 0 && self.attempt >= self.max_retries {
            return None;
        }

        let delay_ms = self.base_delay.as_millis() as f64
            * self.multiplier.powi(self.attempt as i32);
        let delay = Duration::from_millis(delay_ms as u64).min(self.max_delay);

        self.attempt += 1;
        Some(delay)
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn attempt(&self) -> u64 {
        self.attempt
    }
}

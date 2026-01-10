#![allow(dead_code)]
use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{warn, info};

pub struct RetryPolicy {
    max_retries: usize,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl RetryPolicy {
    pub fn new(max_retries: usize, initial_backoff_ms: u64, max_backoff_ms: u64) -> Self {
        Self {
            max_retries,
            initial_backoff: Duration::from_millis(initial_backoff_ms),
            max_backoff: Duration::from_millis(max_backoff_ms),
        }
    }

    /// Retry a future with exponential backoff
    pub async fn retry<F, Fut, T, E>(&self, operation_name: &str, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut attempt = 0;
        let mut backoff = self.initial_backoff;

        loop {
            match f().await {
                Ok(result) => {
                    if attempt > 0 {
                        info!(
                            operation = operation_name,
                            attempts = attempt + 1,
                            "Operation succeeded after retries"
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    attempt += 1;
                    if attempt > self.max_retries {
                        warn!(
                            operation = operation_name,
                            attempts = attempt,
                            error = %e,
                            "Operation failed after max retries"
                        );
                        return Err(e);
                    }

                    warn!(
                        operation = operation_name,
                        attempt = attempt,
                        max_retries = self.max_retries,
                        backoff_ms = backoff.as_millis(),
                        error = %e,
                        "Operation failed, retrying"
                    );

                    sleep(backoff).await;

                    // Exponential backoff with jitter
                    backoff = std::cmp::min(
                        backoff * 2,
                        self.max_backoff
                    );
                }
            }
        }
    }
}
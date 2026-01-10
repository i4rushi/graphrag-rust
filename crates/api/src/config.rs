use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub mode: OperationMode,
    pub concurrency: ConcurrencyConfig,
    pub retry: RetryConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OperationMode {
    Fast,      // Use cached results aggressively, lower quality LLM
    Accurate,  // Always fresh, best quality LLM
    Balanced,  // Default: cache when available, good quality
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    pub max_concurrent_llm_calls: usize,
    pub max_concurrent_extractions: usize,
    pub request_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: OperationMode::Balanced,
            concurrency: ConcurrencyConfig {
                max_concurrent_llm_calls: 3,
                max_concurrent_extractions: 5,
                request_timeout_secs: 60,
            },
            retry: RetryConfig {
                max_retries: 3,
                initial_backoff_ms: 1000,
                max_backoff_ms: 10000,
            },
            cache: CacheConfig {
                enabled: true,
                max_entries: 10000,
            },
        }
    }
}

impl AppConfig {
    #![allow(dead_code)]
    pub fn fast_mode() -> Self {
        Self {
            mode: OperationMode::Fast,
            concurrency: ConcurrencyConfig {
                max_concurrent_llm_calls: 10,
                max_concurrent_extractions: 20,
                request_timeout_secs: 30,
            },
            retry: RetryConfig {
                max_retries: 2,
                initial_backoff_ms: 500,
                max_backoff_ms: 5000,
            },
            cache: CacheConfig {
                enabled: true,
                max_entries: 50000,
            },
        }
    }

    pub fn accurate_mode() -> Self {
        Self {
            mode: OperationMode::Accurate,
            concurrency: ConcurrencyConfig {
                max_concurrent_llm_calls: 2,
                max_concurrent_extractions: 3,
                request_timeout_secs: 120,
            },
            retry: RetryConfig {
                max_retries: 5,
                initial_backoff_ms: 2000,
                max_backoff_ms: 20000,
            },
            cache: CacheConfig {
                enabled: false,
                max_entries: 0,
            },
        }
    }
}
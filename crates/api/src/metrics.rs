use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

pub struct Metrics {
    // Counters
    total_requests: AtomicUsize,
    successful_requests: AtomicUsize,
    failed_requests: AtomicUsize,

    // Timing (in microseconds)
    total_ingest_time_us: AtomicU64,
    total_extract_time_us: AtomicU64,
    total_index_time_us: AtomicU64,
    total_query_time_us: AtomicU64,

    // Counts
    total_chunks_processed: AtomicUsize,
    total_entities_extracted: AtomicUsize,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            total_requests: AtomicUsize::new(0),
            successful_requests: AtomicUsize::new(0),
            failed_requests: AtomicUsize::new(0),
            total_ingest_time_us: AtomicU64::new(0),
            total_extract_time_us: AtomicU64::new(0),
            total_index_time_us: AtomicU64::new(0),
            total_query_time_us: AtomicU64::new(0),
            total_chunks_processed: AtomicUsize::new(0),
            total_entities_extracted: AtomicUsize::new(0),
        })
    }

    pub fn record_request(&self, success: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_ingest(&self, duration: std::time::Duration, chunks: usize) {
        self.total_ingest_time_us.fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
        self.total_chunks_processed.fetch_add(chunks, Ordering::Relaxed);
    }

    pub fn record_extract(&self, duration: std::time::Duration, entities: usize) {
        self.total_extract_time_us.fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
        self.total_entities_extracted.fetch_add(entities, Ordering::Relaxed);
    }

    pub fn record_index(&self, duration: std::time::Duration) {
        self.total_index_time_us.fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    pub fn record_query(&self, duration: std::time::Duration) {
        self.total_query_time_us.fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            successful_requests: self.successful_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            avg_ingest_time_ms: self.avg_time_ms(&self.total_ingest_time_us, &self.total_chunks_processed),
            avg_extract_time_ms: self.avg_time_ms(&self.total_extract_time_us, &self.total_entities_extracted),
            avg_index_time_ms: self.avg_time_ms(&self.total_index_time_us, &AtomicUsize::new(1)),
            avg_query_time_ms: self.avg_time_ms(&self.total_query_time_us, &self.total_requests),
            total_chunks_processed: self.total_chunks_processed.load(Ordering::Relaxed),
            total_entities_extracted: self.total_entities_extracted.load(Ordering::Relaxed),
        }
    }

    fn avg_time_ms(&self, total_us: &AtomicU64, count: &AtomicUsize) -> f64 {
        let total = total_us.load(Ordering::Relaxed) as f64;
        let cnt = count.load(Ordering::Relaxed) as f64;
        if cnt > 0.0 {
            total / cnt / 1000.0 // Convert to ms
        } else {
            0.0
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub avg_ingest_time_ms: f64,
    pub avg_extract_time_ms: f64,
    pub avg_index_time_ms: f64,
    pub avg_query_time_ms: f64,
    pub total_chunks_processed: usize,
    pub total_entities_extracted: usize,
}

pub struct TimedOperation {
    start: Instant,
}

impl TimedOperation {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}
//! Metrics recording and tracing initialization for the bap runtime.
//! Collectors share state behind a mutex; snapshots clone it for reporting.

#![forbid(unsafe_code)]

pub mod logging;
mod metrics;

pub use metrics::{HeapSample, MetricsCollector, MetricsSnapshot, OpStat};

use hdrhistogram::Histogram;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Timing totals for one named op.
/// Durations are wall-clock nanoseconds.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct OpStat {
    pub calls: u64,
    pub total_ns: u128,
    pub max_ns: u128,
    /// 50th percentile, present only for async ops.
    pub p50_ns: Option<u128>,
    /// 99th percentile, present only for async ops.
    pub p99_ns: Option<u128>,
}

/// One heap measurement with an elapsed time from the collector epoch.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct HeapSample {
    pub elapsed_ms: u128,
    pub used_bytes: usize,
    pub total_bytes: usize,
}

/// Clone of the collected state at one point in time.
/// `ops` keys sort lexically for deterministic reports.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MetricsSnapshot {
    pub cold_start_ms: f64,
    pub ops: BTreeMap<String, OpStat>,
    pub heap_samples: Vec<HeapSample>,
}

/// Upper bound for latency histograms, in microseconds (~16 minutes).
const HISTOGRAM_MAX_US: u64 = 1_000_000_000;

#[derive(Debug)]
struct Shared {
    epoch: Instant,
    cold_start_ms: f64,
    ops: BTreeMap<String, OpStat>,
    heap_samples: Vec<HeapSample>,
    histograms: BTreeMap<String, Histogram<u64>>,
}

/// Thread-safe metrics store shared through an `Arc<Mutex<_>>`.
/// Clones reference the same underlying state.
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    inner: Arc<Mutex<Shared>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Locks the shared state, recovering from a poisoned mutex.
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Creates a collector with empty state and a fresh epoch.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Shared {
                epoch: Instant::now(),
                cold_start_ms: 0.0,
                ops: BTreeMap::new(),
                heap_samples: Vec::new(),
                histograms: BTreeMap::new(),
            })),
        }
    }

    /// Sets `cold_start_ms` to `duration` expressed in milliseconds.
    pub fn record_cold_start(&self, duration: Duration) {
        self.lock().cold_start_ms = duration.as_secs_f64() * 1000.0;
    }

    /// Adds one call of `name` with the given duration.
    /// Increments the call count and rolls up total and max nanoseconds.
    pub fn record_op(&self, name: &str, duration: Duration) {
        let mut shared = self.lock();
        let stat = shared.ops.entry(name.to_owned()).or_default();
        stat.calls += 1;
        let ns = duration.as_nanos();
        stat.total_ns += ns;
        stat.max_ns = stat.max_ns.max(ns);
    }

    /// Records an async op call, additionally feeding a latency histogram
    /// so snapshots can report p50/p99 percentiles for the name.
    pub fn record_op_async(&self, name: &str, duration: Duration) {
        let mut shared = self.lock();
        let stat = shared.ops.entry(name.to_owned()).or_default();
        stat.calls += 1;
        let ns = duration.as_nanos();
        stat.total_ns += ns;
        stat.max_ns = stat.max_ns.max(ns);
        let us = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        shared
            .histograms
            .entry(name.to_owned())
            .or_insert_with(|| {
                Histogram::new_with_max(HISTOGRAM_MAX_US, 3_u8).expect("valid bounds")
            })
            .saturating_record(us);
    }

    /// Appends a heap sample with the given byte counts.
    /// The sample's `elapsed_ms` is measured from the collector epoch.
    pub fn record_heap(&self, used_bytes: usize, total_bytes: usize) {
        let mut shared = self.lock();
        let elapsed_ms = shared.epoch.elapsed().as_millis();
        shared.heap_samples.push(HeapSample {
            elapsed_ms,
            used_bytes,
            total_bytes,
        });
    }

    /// Clones the current state into a `MetricsSnapshot`.
    /// Async-op percentiles are computed from the live histograms.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let shared = self.lock();
        let mut ops = shared.ops.clone();
        for (name, stat) in ops.iter_mut() {
            if let Some(hist) = shared.histograms.get(name) {
                stat.p50_ns = Some(hist.value_at_quantile(0.5) as u128 * 1_000);
                stat.p99_ns = Some(hist.value_at_quantile(0.99) as u128 * 1_000);
            }
        }
        MetricsSnapshot {
            cold_start_ms: shared.cold_start_ms,
            ops,
            heap_samples: shared.heap_samples.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_op_accumulates() {
        let collector = MetricsCollector::new();
        collector.record_op("console.log", Duration::from_millis(2));
        collector.record_op("console.log", Duration::from_millis(5));
        let snap = collector.snapshot();
        let op = &snap.ops["console.log"];
        assert_eq!(op.calls, 2);
        assert_eq!(op.total_ns, 7_000_000);
        assert_eq!(op.max_ns, 5_000_000);
        assert_eq!(op.p50_ns, None);
        assert_eq!(op.p99_ns, None);
        assert!(snap.cold_start_ms >= 0.0);
    }

    #[test]
    fn record_op_async_reports_percentiles() {
        let collector = MetricsCollector::new();
        for ms in [10, 20, 100, 200, 300] {
            collector.record_op_async("net.read", Duration::from_millis(ms));
        }
        let snap = collector.snapshot();
        let op = &snap.ops["net.read"];
        assert_eq!(op.calls, 5);
        assert_eq!(op.total_ns, 630_000_000);
        assert_eq!(op.max_ns, 300_000_000);
        let p50 = op.p50_ns.expect("p50 present");
        let p99 = op.p99_ns.expect("p99 present");
        assert!((99_000_000..=101_000_000).contains(&p50), "p50={p50}");
        assert!((290_000_000..=310_000_000).contains(&p99), "p99={p99}");
    }

    #[test]
    fn record_heap_saves_samples() {
        let collector = MetricsCollector::new();
        collector.record_heap(10, 100);
        collector.record_heap(20, 100);
        let snap = collector.snapshot();
        assert_eq!(snap.heap_samples.len(), 2);
        assert_eq!(snap.heap_samples[0].used_bytes, 10);
        assert!(snap.heap_samples[1].elapsed_ms >= snap.heap_samples[0].elapsed_ms);
    }
}

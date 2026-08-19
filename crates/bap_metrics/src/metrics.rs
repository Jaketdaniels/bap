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

#[derive(Debug)]
struct Shared {
    epoch: Instant,
    cold_start_ms: f64,
    ops: BTreeMap<String, OpStat>,
    heap_samples: Vec<HeapSample>,
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
    pub fn snapshot(&self) -> MetricsSnapshot {
        let shared = self.lock();
        MetricsSnapshot {
            cold_start_ms: shared.cold_start_ms,
            ops: shared.ops.clone(),
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
        assert!(snap.cold_start_ms >= 0.0);
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

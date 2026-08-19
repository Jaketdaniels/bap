use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct OpStat {
    pub calls: u64,
    pub total_ns: u128,
    pub max_ns: u128,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HeapSample {
    pub elapsed_ms: u128,
    pub used_bytes: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MetricsSnapshot {
    pub cold_start_ms: f64,
    pub ops: HashMap<String, OpStat>,
    pub heap_samples: Vec<HeapSample>,
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    started: Instant,
    inner: Arc<Mutex<MetricsSnapshot>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            inner: Arc::new(Mutex::new(MetricsSnapshot::default())),
        }
    }

    pub fn record_op(&self, name: &str, duration: Duration) {
        let mut snap = self.inner.lock().expect("metrics lock poisoned");
        let stat = snap.ops.entry(name.to_owned()).or_default();
        stat.calls += 1;
        stat.total_ns += duration.as_nanos();
        stat.max_ns = stat.max_ns.max(duration.as_nanos());
    }

    pub fn record_heap(&self, used_bytes: usize, total_bytes: usize) {
        let mut snap = self.inner.lock().expect("metrics lock poisoned");
        snap.heap_samples.push(HeapSample {
            elapsed_ms: self.started.elapsed().as_millis(),
            used_bytes,
            total_bytes,
        });
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut snap = self.inner.lock().expect("metrics lock poisoned").clone();
        snap.cold_start_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        snap
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

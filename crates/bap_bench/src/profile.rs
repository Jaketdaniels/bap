//! Per-request profiling primitives: latency histograms and sample records.

use hdrhistogram::Histogram;
use std::time::Duration;

/// Upper bound for route-latency histograms: 60 seconds in nanoseconds.
const HISTOGRAM_MAX_NS: u64 = 60_000_000_000;

/// One executed request on one lane.
#[derive(Clone, Copy, Debug)]
pub struct RequestSample {
    /// Long-lived isolate that executed the request.
    pub lane_id: u32,
    /// Zero-based position within its phase (warmup or measured).
    pub index: u32,
    pub duration_ns: u64,
    /// True for the first invocation on a lane.
    pub cold: bool,
    /// Whether this sample belongs to the measured phase.
    pub measured: bool,
    /// `(used, total)` isolate heap bytes captured before execution.
    pub heap_before: (usize, usize),
    /// Same capture after execution completed.
    pub heap_after: (usize, usize),
}

/// Wraps a route invocation, capturing duration and heap movement.
#[derive(Debug)]
pub struct ProfileScope {
    lane_id: u32,
    index: u32,
    cold: bool,
    measured: bool,
    started: std::time::Instant,
    heap_before: (usize, usize),
}

impl ProfileScope {
    pub fn begin(lane_id: u32, index: u32, cold: bool, measured: bool) -> Self {
        Self {
            lane_id,
            index,
            cold,
            measured,
            started: std::time::Instant::now(),
            heap_before: (0, 0),
        }
    }

    /// Records the isolate heap before execution; call right after begin.
    pub fn observe_heap(&mut self, used: usize, total: usize) {
        self.heap_before = (used, total);
    }

    /// Completes the scope into a sample using the heap captured at end.
    pub fn finish(self, heap_after: (usize, usize)) -> RequestSample {
        RequestSample {
            lane_id: self.lane_id,
            index: self.index,
            duration_ns: u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX),
            cold: self.cold,
            measured: self.measured,
            heap_before: self.heap_before,
            heap_after,
        }
    }

    /// Elapsed so far, for callers that interleave work mid-scope.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

/// Nanosecond latency histogram exposing spec-shaped percentiles.
#[derive(Debug)]
pub struct LatencyHistogram {
    inner: Histogram<u64>,
}

impl LatencyHistogram {
    pub fn new() -> Self {
        Self {
            inner: Histogram::new_with_max(HISTOGRAM_MAX_NS, 3).expect("valid histogram bounds"),
        }
    }

    pub fn record(&mut self, duration_ns: u64) {
        self.inner.saturating_record(duration_ns);
    }

    pub fn extend(&mut self, other: &Self) {
        let snapshot = other.inner.clone();
        self.inner.add(&snapshot).expect("identical bounds");
    }

    pub fn count(&self) -> u64 {
        self.inner.len()
    }

    pub fn p50(&self) -> u64 {
        self.inner.value_at_quantile(0.5)
    }

    pub fn p99(&self) -> u64 {
        self.inner.value_at_quantile(0.99)
    }

    pub fn min(&self) -> u64 {
        self.inner.min()
    }

    pub fn max(&self) -> u64 {
        self.inner.max()
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Classifies an invocation: the first call on a lane is cold.
pub const fn is_cold(index: u32) -> bool {
    index == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_distribution_hits_percentiles_within_tolerance() {
        let mut histogram = LatencyHistogram::new();
        // Uniform samples across 1..=1000 ms; true p50 ~500 ms, p99 ~990 ms.
        for ms in 1..=1000_u64 {
            histogram.record(ms * 1_000_000);
        }
        let p50 = histogram.p50();
        let p99 = histogram.p99();
        assert!(
            (490_000_000..=530_000_000).contains(&p50),
            "p50 was {p50} ns"
        );
        assert!(
            (980_000_000..=1_010_000_000).contains(&p99),
            "p99 was {p99} ns"
        );
        assert_eq!(histogram.count(), 1000);
    }

    #[test]
    fn merge_combines_counts_and_bounds() {
        let mut left = LatencyHistogram::new();
        left.record(1_000);
        left.record(2_000);
        let mut right = LatencyHistogram::new();
        right.record(9_000);
        left.extend(&right);
        assert_eq!(left.count(), 3);
        // Three significant digits round the tracked maximum up to its bucket.
        assert!(
            (9_000..=9_100).contains(&left.max()),
            "max was {}",
            left.max()
        );
        assert_eq!(left.min(), 1_000);
    }

    #[test]
    fn first_invocation_is_cold() {
        assert!(is_cold(0));
        assert!(!is_cold(1));
        assert!(!is_cold(42));
    }

    #[test]
    fn scope_captures_duration_and_heap_movement() {
        let mut scope = ProfileScope::begin(7, 3, true, false);
        scope.observe_heap(100, 200);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let sample = scope.finish((150, 220));
        assert_eq!(sample.lane_id, 7);
        assert_eq!(sample.index, 3);
        assert!(sample.cold);
        assert!(!sample.measured);
        assert_eq!(sample.heap_before, (100, 200));
        assert_eq!(sample.heap_after, (150, 220));
        assert!(sample.duration_ns >= 2_000_000 - 500_000);
    }
}

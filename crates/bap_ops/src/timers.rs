//! Timer ops backed by tokio, surfaced to JS as promises.

use super::metrics::{record_op, record_op_async};
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Registry of pending timer ids so `clearTimeout` can suppress callbacks.
#[derive(Default)]
pub(crate) struct TimerState {
    next_id: u64,
    cancelled: HashSet<u64>,
}

impl TimerState {
    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// Allocates a timer id, tracked so `clearTimeout` can suppress the callback.
#[op2(fast)]
#[number]
pub fn op_timer_create(state: &mut OpState) -> u64 {
    let started = Instant::now();
    let id = state.borrow_mut::<TimerState>().allocate_id();
    record_op(state, "timers.create", started);
    id
}

/// Marks `id` as cancelled so a pending timer does not fire its callback.
#[op2(fast)]
pub fn op_timer_clear(state: &mut OpState, #[number] id: u64) {
    let started = Instant::now();
    state.borrow_mut::<TimerState>().cancelled.insert(id);
    record_op(state, "timers.clear", started);
}

/// Sleeps `delay_ms`, then reports whether the timer is still armed.
#[op2(async(lazy), fast)]
pub async fn op_timer_set_timeout(
    state: Rc<RefCell<OpState>>,
    #[number] id: u64,
    #[number] delay_ms: u64,
) -> Result<bool, JsErrorBox> {
    let started = Instant::now();
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    let armed = !state
        .borrow()
        .borrow::<TimerState>()
        .cancelled
        .contains(&id);
    record_op_async(&state.borrow(), "timers.setTimeout", started);
    Ok(armed)
}

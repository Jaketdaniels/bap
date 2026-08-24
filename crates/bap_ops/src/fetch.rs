use super::metrics::record_op_async;
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

/// In-flight response bodies, keyed by the id returned from `op_fetch`.
pub struct FetchState {
    next_id: u64,
    bodies: HashMap<u64, Vec<u8>>,
}

impl Default for FetchState {
    fn default() -> Self {
        Self {
            next_id: 1,
            bodies: HashMap::new(),
        }
    }
}

impl FetchState {
    fn add_body(&mut self, body: Vec<u8>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.bodies.insert(id, body);
        id
    }

    fn take_body(&mut self, id: u64) -> Option<Vec<u8>> {
        self.bodies.remove(&id)
    }
}

/// Response metadata; the body bytes stay in `FetchState` until requested.
#[derive(Serialize)]
pub struct FetchResult {
    pub id: u64,
    pub status: u16,
    pub ok: bool,
    pub headers: Vec<(String, String)>,
}

/// Maps a `reqwest::Error` to a JS error prefixed with `fetch:`.
fn fetch_err(err: reqwest::Error) -> JsErrorBox {
    JsErrorBox::new("Error", format!("fetch: {err}"))
}

/// Sends a GET request, resolving to `{id, status, ok, headers}`.
#[op2(async(lazy), fast)]
#[serde]
pub async fn op_fetch(
    state: Rc<RefCell<OpState>>,
    #[string] url: String,
) -> Result<FetchResult, JsErrorBox> {
    let started = Instant::now();
    let client = reqwest::Client::builder()
        .user_agent(concat!("bap/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| JsErrorBox::new("Error", format!("fetch: {e}")))?;
    let response = client.get(&url).send().await.map_err(fetch_err)?;
    let status = response.status().as_u16();
    let ok = response.status().is_success();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    let body = response.bytes().await.map_err(fetch_err)?.to_vec();
    let id = state.borrow_mut().borrow_mut::<FetchState>().add_body(body);
    record_op_async(&state.borrow(), "fetch.request", started);
    Ok(FetchResult {
        id,
        status,
        ok,
        headers,
    })
}

/// Returns the body for a fetch id as a `Uint8Array`, consuming it.
#[op2(async(lazy), fast)]
#[buffer]
pub async fn op_fetch_body(
    state: Rc<RefCell<OpState>>,
    #[number] id: u64,
) -> Result<Vec<u8>, JsErrorBox> {
    let started = Instant::now();
    let body = state.borrow_mut().borrow_mut::<FetchState>().take_body(id);
    record_op_async(&state.borrow(), "fetch.body", started);
    body.ok_or_else(|| JsErrorBox::new("Error", "fetch: unknown body"))
}

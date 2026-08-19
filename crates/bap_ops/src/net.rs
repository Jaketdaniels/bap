//! TCP ops backed by tokio, with sockets held in an OpState resource table.

use super::metrics::{record_op, record_op_async};
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Sockets and listeners keyed by the ids returned to JS.
#[derive(Default)]
pub(crate) struct NetState {
    next_id: u64,
    listeners: HashMap<u64, tokio::net::TcpListener>,
    conns: HashMap<u64, tokio::net::TcpStream>,
}

impl NetState {
    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_listener(&mut self, listener: tokio::net::TcpListener) -> u64 {
        let id = self.next_id();
        self.listeners.insert(id, listener);
        id
    }

    fn insert_listener(&mut self, id: u64, listener: tokio::net::TcpListener) {
        self.listeners.insert(id, listener);
    }

    fn take_listener(&mut self, id: u64) -> Option<tokio::net::TcpListener> {
        self.listeners.remove(&id)
    }

    fn add_conn(&mut self, stream: tokio::net::TcpStream) -> u64 {
        let id = self.next_id();
        self.conns.insert(id, stream);
        id
    }

    fn insert_conn(&mut self, id: u64, stream: tokio::net::TcpStream) {
        self.conns.insert(id, stream);
    }

    fn take_conn(&mut self, id: u64) -> Option<tokio::net::TcpStream> {
        self.conns.remove(&id)
    }
}

/// Handle for a listening server, with the actually bound port.
#[derive(Serialize)]
pub struct ListenResult {
    pub id: u64,
    pub port: u16,
}

/// Maps an `io::Error` to a JS error prefixed with `net:`.
fn net_err(err: std::io::Error) -> JsErrorBox {
    JsErrorBox::new("Error", format!("net: {err}"))
}

/// Binds a listener on `127.0.0.1`, resolving to `{id, port}`.
#[op2(async(lazy), fast)]
#[serde]
pub async fn op_net_listen(
    state: Rc<RefCell<OpState>>,
    #[smi] port: u16,
) -> Result<ListenResult, JsErrorBox> {
    let started = Instant::now();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(net_err)?;
    let bound = listener.local_addr().map_err(net_err)?.port();
    let id = state
        .borrow_mut()
        .borrow_mut::<NetState>()
        .add_listener(listener);
    record_op_async(&state.borrow(), "net.listen", started);
    Ok(ListenResult { id, port: bound })
}

/// Accepts one connection, returning the new connection id.
#[op2(async(lazy), fast)]
#[number]
pub async fn op_net_accept(
    state: Rc<RefCell<OpState>>,
    #[number] listener_id: u64,
) -> Result<u64, JsErrorBox> {
    let started = Instant::now();
    let listener = state
        .borrow_mut()
        .borrow_mut::<NetState>()
        .take_listener(listener_id)
        .ok_or_else(unknown_listener)?;
    let accepted = listener.accept().await;
    state
        .borrow_mut()
        .borrow_mut::<NetState>()
        .insert_listener(listener_id, listener);
    match accepted {
        Ok((stream, _)) => {
            let id = state.borrow_mut().borrow_mut::<NetState>().add_conn(stream);
            record_op_async(&state.borrow(), "net.accept", started);
            Ok(id)
        }
        Err(err) => Err(net_err(err)),
    }
}

/// Opens an outbound connection, returning the new connection id.
#[op2(async(lazy), fast)]
#[number]
pub async fn op_net_connect(
    state: Rc<RefCell<OpState>>,
    #[string] host: String,
    #[smi] port: u16,
) -> Result<u64, JsErrorBox> {
    let started = Instant::now();
    let stream = tokio::net::TcpStream::connect((host.as_str(), port))
        .await
        .map_err(net_err)?;
    let id = state.borrow_mut().borrow_mut::<NetState>().add_conn(stream);
    record_op_async(&state.borrow(), "net.connect", started);
    Ok(id)
}

/// Reads one chunk from a connection, resolving to a `Uint8Array`.
/// An empty buffer signals the peer closed the connection.
#[op2(async(lazy), fast)]
#[buffer]
pub async fn op_net_read(
    state: Rc<RefCell<OpState>>,
    #[number] conn_id: u64,
) -> Result<Vec<u8>, JsErrorBox> {
    let started = Instant::now();
    let mut stream = state
        .borrow_mut()
        .borrow_mut::<NetState>()
        .take_conn(conn_id)
        .ok_or_else(unknown_conn)?;
    let mut buf = vec![0u8; 4096];
    let read = stream.read(&mut buf).await;
    state
        .borrow_mut()
        .borrow_mut::<NetState>()
        .insert_conn(conn_id, stream);
    let n = read.map_err(net_err)?;
    buf.truncate(n);
    record_op_async(&state.borrow(), "net.read", started);
    Ok(buf)
}

/// Writes `data` to a connection, resolving to the bytes written.
#[op2(async(lazy))]
#[number]
pub async fn op_net_write(
    state: Rc<RefCell<OpState>>,
    #[number] conn_id: u64,
    #[buffer] data: JsBuffer,
) -> Result<usize, JsErrorBox> {
    let started = Instant::now();
    let mut stream = state
        .borrow_mut()
        .borrow_mut::<NetState>()
        .take_conn(conn_id)
        .ok_or_else(unknown_conn)?;
    let write = stream.write(data.as_ref()).await;
    state
        .borrow_mut()
        .borrow_mut::<NetState>()
        .insert_conn(conn_id, stream);
    let n = write.map_err(net_err)?;
    record_op_async(&state.borrow(), "net.write", started);
    Ok(n)
}

/// Closes a connection, dropping its socket.
#[op2(fast)]
pub fn op_net_close(state: &mut OpState, #[number] conn_id: u64) {
    let started = Instant::now();
    state.borrow_mut::<NetState>().conns.remove(&conn_id);
    record_op(state, "net.close", started);
}

/// Closes a listener, dropping its socket.
#[op2(fast)]
pub fn op_net_close_listener(state: &mut OpState, #[number] listener_id: u64) {
    let started = Instant::now();
    state
        .borrow_mut::<NetState>()
        .listeners
        .remove(&listener_id);
    record_op(state, "net.closeListener", started);
}

fn unknown_conn() -> JsErrorBox {
    JsErrorBox::new("Error", "net: unknown connection")
}

fn unknown_listener() -> JsErrorBox {
    JsErrorBox::new("Error", "net: unknown listener")
}

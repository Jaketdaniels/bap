//! File I/O ops backed by tokio::fs, exposed as JS promises.
//! Sync variants back the CJS loader and the node:fs shim.

use super::metrics::{record_op, record_op_async};
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

/// One directory entry with its file type flags.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub is_file: bool,
    pub is_dir: bool,
}

/// Size, modification time, and type flags for a path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub size: u64,
    pub modified_ms: i64,
    pub is_file: bool,
    pub is_dir: bool,
}

/// Maps an `io::Error` to a JS error naming the offending path.
fn io_err(path: &str, err: std::io::Error) -> JsErrorBox {
    JsErrorBox::new("Error", format!("{path}: {err}"))
}

/// Reads a file's bytes, resolving to a `Uint8Array`.
#[op2(async(lazy), fast)]
#[buffer]
pub async fn op_fs_read_file(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let started = Instant::now();
    let data = tokio::fs::read(&path).await.map_err(|e| io_err(&path, e))?;
    record_op_async(&state.borrow(), "fs.readFile", started);
    Ok(data)
}

/// Writes `data` to `path`, creating or truncating the file.
#[op2(async(lazy))]
pub async fn op_fs_write_file(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
    #[buffer] data: JsBuffer,
) -> Result<(), JsErrorBox> {
    let started = Instant::now();
    tokio::fs::write(&path, &data)
        .await
        .map_err(|e| io_err(&path, e))?;
    record_op_async(&state.borrow(), "fs.writeFile", started);
    Ok(())
}

/// Lists a directory, resolving to an array of `{name, isFile, isDir}`.
#[op2(async(lazy), fast)]
#[serde]
pub async fn op_fs_read_dir(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<DirEntry>, JsErrorBox> {
    let started = Instant::now();
    let mut entries = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| io_err(&path, e))?;
    let mut out = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(|e| io_err(&path, e))? {
        let ty = entry.file_type().await.map_err(|e| io_err(&path, e))?;
        out.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_file: ty.is_file(),
            is_dir: ty.is_dir(),
        });
    }
    record_op_async(&state.borrow(), "fs.readDir", started);
    Ok(out)
}

/// Stats a path, resolving to `{size, modified_ms}` (epoch milliseconds).
#[op2(async(lazy), fast)]
#[serde]
pub async fn op_fs_stat(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<FileStat, JsErrorBox> {
    let started = Instant::now();
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| io_err(&path, e))?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let is_dir = meta.is_dir();
    record_op_async(&state.borrow(), "fs.stat", started);
    Ok(FileStat {
        size: meta.len(),
        modified_ms,
        is_file: !is_dir,
        is_dir,
    })
}

/// Reads a file's bytes synchronously; backs `require` and `fs.readFileSync`.
#[op2]
#[buffer]
pub fn op_fs_read_file_sync(
    state: &mut OpState,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let started = Instant::now();
    let data = std::fs::read(&path).map_err(|e| io_err(&path, e))?;
    record_op(state, "fs.readFileSync", started);
    Ok(data)
}

/// Writes `data` to `path` synchronously.
#[op2]
pub fn op_fs_write_file_sync(
    state: &mut OpState,
    #[string] path: String,
    #[buffer] data: JsBuffer,
) -> Result<(), JsErrorBox> {
    let started = Instant::now();
    std::fs::write(&path, &data).map_err(|e| io_err(&path, e))?;
    record_op(state, "fs.writeFileSync", started);
    Ok(())
}

/// Reports whether a path exists synchronously.
#[op2(fast)]
pub fn op_fs_exists_sync(state: &mut OpState, #[string] path: String) -> bool {
    let started = Instant::now();
    let exists = std::fs::symlink_metadata(&path).is_ok();
    record_op(state, "fs.existsSync", started);
    exists
}

/// Lists a directory synchronously into `{name, isFile, isDir}` entries.
#[op2]
#[serde]
pub fn op_fs_read_dir_sync(
    state: &mut OpState,
    #[string] path: String,
) -> Result<Vec<DirEntry>, JsErrorBox> {
    let started = Instant::now();
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| io_err(&path, e))? {
        let entry = entry.map_err(|e| io_err(&path, e))?;
        let ty = entry.file_type().map_err(|e| io_err(&path, e))?;
        out.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_file: ty.is_file(),
            is_dir: ty.is_dir(),
        });
    }
    record_op(state, "fs.readDirSync", started);
    Ok(out)
}

/// Stats a path synchronously into `{size, modifiedMs}`.
#[op2]
#[serde]
pub fn op_fs_stat_sync(
    state: &mut OpState,
    #[string] path: String,
) -> Result<FileStat, JsErrorBox> {
    let started = Instant::now();
    let meta = std::fs::metadata(&path).map_err(|e| io_err(&path, e))?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let is_dir = meta.is_dir();
    record_op(state, "fs.statSync", started);
    Ok(FileStat {
        size: meta.len(),
        modified_ms,
        is_file: !is_dir,
        is_dir,
    })
}

/// Creates a directory (and parents) synchronously.
#[op2(fast)]
pub fn op_fs_make_dir_sync(state: &mut OpState, #[string] path: String) -> Result<(), JsErrorBox> {
    let started = Instant::now();
    std::fs::create_dir_all(&path).map_err(|e| io_err(&path, e))?;
    record_op(state, "fs.mkdirSync", started);
    Ok(())
}

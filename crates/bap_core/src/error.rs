//! Kind-labeled runtime errors wrapping an underlying cause.

use std::error::Error as StdError;
use std::fmt;

/// Classifies which stage of an operation produced the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Io,
    Load,
    Eval,
}

impl ErrorKind {
    /// Returns the lowercase label used in `Display` output.
    fn label(self) -> &'static str {
        match self {
            ErrorKind::Io => "io",
            ErrorKind::Load => "load",
            ErrorKind::Eval => "eval",
        }
    }
}

/// An error tagged with an `ErrorKind` and an underlying `AnyError` cause.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    inner: deno_core::error::AnyError,
}

impl Error {
    /// Wraps `err` with kind `Io`.
    pub fn io(err: impl Into<deno_core::error::AnyError>) -> Self {
        Self::new(ErrorKind::Io, err)
    }

    /// Wraps `err` with kind `Load`.
    pub fn load(err: impl Into<deno_core::error::AnyError>) -> Self {
        Self::new(ErrorKind::Load, err)
    }

    /// Wraps `err` with kind `Eval`.
    pub fn eval(err: impl Into<deno_core::error::AnyError>) -> Self {
        Self::new(ErrorKind::Eval, err)
    }

    /// Builds `Error` from a kind and any error convertible to `AnyError`.
    fn new(kind: ErrorKind, err: impl Into<deno_core::error::AnyError>) -> Self {
        Self {
            kind,
            inner: err.into(),
        }
    }

    /// Returns the classifying kind.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.label(), self.inner)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.inner.as_ref())
    }
}

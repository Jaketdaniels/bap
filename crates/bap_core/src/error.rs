use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Io,
    Load,
    Eval,
}

impl ErrorKind {
    fn label(self) -> &'static str {
        match self {
            ErrorKind::Io => "io",
            ErrorKind::Load => "load",
            ErrorKind::Eval => "eval",
        }
    }
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    inner: deno_core::error::AnyError,
}

impl Error {
    pub fn io(err: impl Into<deno_core::error::AnyError>) -> Self {
        Self::new(ErrorKind::Io, err)
    }

    pub fn load(err: impl Into<deno_core::error::AnyError>) -> Self {
        Self::new(ErrorKind::Load, err)
    }

    pub fn eval(err: impl Into<deno_core::error::AnyError>) -> Self {
        Self::new(ErrorKind::Eval, err)
    }

    fn new(kind: ErrorKind, err: impl Into<deno_core::error::AnyError>) -> Self {
        Self {
            kind,
            inner: err.into(),
        }
    }

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

//! Audited numeric primitives with explicit failure modes.

#![forbid(unsafe_code)]

mod checked;
mod error;
mod fixed;
mod interval;

pub use checked::{CheckedI32, CheckedI64};
pub use error::NumericError;
pub use fixed::{Fixed, Fixed18_4};
pub use interval::Interval;

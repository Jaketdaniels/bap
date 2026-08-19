use std::fmt;

/// A numeric-domain failure with a stable public code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericError {
    InvalidArgument,
    InvalidConfiguration,
    Indeterminate,
    Discontinuous,
    DivisionByZero,
    Overflow,
}

impl NumericError {
    /// Returns the stable code used across language boundaries.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidArgument => "BAP_NUMERIC_INVALID_ARGUMENT",
            Self::InvalidConfiguration => "BAP_NUMERIC_INVALID_CONFIGURATION",
            Self::Indeterminate => "BAP_NUMERIC_INDETERMINATE",
            Self::Discontinuous => "BAP_NUMERIC_DISCONTINUOUS",
            Self::DivisionByZero => "BAP_NUMERIC_DIVISION_BY_ZERO",
            Self::Overflow => "BAP_NUMERIC_OVERFLOW",
        }
    }
}

impl fmt::Display for NumericError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for NumericError {}

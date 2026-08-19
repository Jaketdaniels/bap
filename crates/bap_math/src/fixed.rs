use crate::NumericError;
use std::{fmt, str::FromStr};

/// A signed fixed-point decimal with total precision `P` and scale `S`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Fixed<const P: u32, const S: u32> {
    coefficient: i128,
}

pub type Fixed18_4 = Fixed<18, 4>;

impl<const P: u32, const S: u32> Fixed<P, S> {
    /// Creates a value from its scaled integer coefficient.
    pub fn new(coefficient: i128) -> Result<Self, NumericError> {
        let max = max_coefficient::<P, S>()?;
        if coefficient.unsigned_abs() > max as u128 {
            return Err(NumericError::Overflow);
        }
        Ok(Self { coefficient })
    }

    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, NumericError> {
        let coefficient = self
            .coefficient
            .checked_add(rhs.coefficient)
            .ok_or(NumericError::Overflow)?;
        Self::new(coefficient)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, NumericError> {
        let coefficient = self
            .coefficient
            .checked_sub(rhs.coefficient)
            .ok_or(NumericError::Overflow)?;
        Self::new(coefficient)
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, NumericError> {
        let product = self
            .coefficient
            .checked_mul(rhs.coefficient)
            .ok_or(NumericError::Overflow)?;
        let coefficient = div_round_half_even(product, scale::<P, S>()?)?;
        Self::new(coefficient)
    }

    pub fn checked_div(self, rhs: Self) -> Result<Self, NumericError> {
        if rhs.coefficient == 0 {
            return Err(NumericError::DivisionByZero);
        }
        let numerator = self
            .coefficient
            .checked_mul(scale::<P, S>()?)
            .ok_or(NumericError::Overflow)?;
        let coefficient = div_round_half_even(numerator, rhs.coefficient)?;
        Self::new(coefficient)
    }

    pub fn checked_neg(self) -> Result<Self, NumericError> {
        let coefficient = self
            .coefficient
            .checked_neg()
            .ok_or(NumericError::Overflow)?;
        Self::new(coefficient)
    }
}

impl<const P: u32, const S: u32> FromStr for Fixed<P, S> {
    type Err = NumericError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let scale = scale::<P, S>()?;
        if input.is_empty() {
            return Err(NumericError::InvalidArgument);
        }

        let (negative, unsigned) = match input.as_bytes()[0] {
            b'+' => (false, &input[1..]),
            b'-' => (true, &input[1..]),
            _ => (false, input),
        };
        if unsigned.is_empty() {
            return Err(NumericError::InvalidArgument);
        }

        let mut parts = unsigned.split('.');
        let integer = parts.next().unwrap_or_default();
        let fractional = parts.next();
        if integer.is_empty()
            || parts.next().is_some()
            || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || fractional.is_some_and(|part| {
                part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(NumericError::InvalidArgument);
        }

        let significant_integer = integer.trim_start_matches('0');
        let integer_digits = (P - S) as usize;
        if significant_integer.len() > integer_digits {
            return Err(NumericError::Overflow);
        }
        let integer_value = if significant_integer.is_empty() {
            0
        } else {
            significant_integer
                .parse::<i128>()
                .map_err(|_| NumericError::Overflow)?
        };
        let mut magnitude = integer_value
            .checked_mul(scale)
            .ok_or(NumericError::Overflow)?;

        if let Some(fractional) = fractional {
            let kept_len = (S as usize).min(fractional.len());
            if kept_len > 0 {
                let kept = fractional[..kept_len]
                    .parse::<i128>()
                    .map_err(|_| NumericError::InvalidArgument)?;
                magnitude = magnitude
                    .checked_add(kept * 10_i128.pow(S - kept_len as u32))
                    .ok_or(NumericError::Overflow)?;
            }
            if fractional.len() > S as usize {
                let discarded = &fractional[S as usize..];
                let first = discarded.as_bytes()[0];
                let above_half = first > b'5'
                    || (first == b'5' && discarded.as_bytes()[1..].iter().any(|&b| b != b'0'));
                let tie = first == b'5' && discarded.as_bytes()[1..].iter().all(|&b| b == b'0');
                if above_half || (tie && magnitude % 2 != 0) {
                    magnitude = magnitude.checked_add(1).ok_or(NumericError::Overflow)?;
                }
            }
        }

        let coefficient = if negative { -magnitude } else { magnitude };
        Self::new(coefficient)
    }
}

impl<const P: u32, const S: u32> fmt::Display for Fixed<P, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.coefficient < 0 {
            formatter.write_str("-")?;
        }
        let magnitude = self.coefficient.unsigned_abs();
        if S == 0 {
            return write!(formatter, "{magnitude}");
        }
        let scale = 10_u128.pow(S);
        write!(
            formatter,
            "{}.{:0width$}",
            magnitude / scale,
            magnitude % scale,
            width = S as usize
        )
    }
}

fn validate_configuration<const P: u32, const S: u32>() -> Result<(), NumericError> {
    if !(1..=18).contains(&P) || S > P {
        return Err(NumericError::InvalidConfiguration);
    }
    Ok(())
}

fn scale<const P: u32, const S: u32>() -> Result<i128, NumericError> {
    validate_configuration::<P, S>()?;
    Ok(10_i128.pow(S))
}

fn max_coefficient<const P: u32, const S: u32>() -> Result<i128, NumericError> {
    validate_configuration::<P, S>()?;
    Ok(10_i128.pow(P) - 1)
}

fn div_round_half_even(numerator: i128, denominator: i128) -> Result<i128, NumericError> {
    let quotient = numerator
        .checked_div(denominator)
        .ok_or(NumericError::Overflow)?;
    let remainder = numerator
        .checked_rem(denominator)
        .ok_or(NumericError::Overflow)?;
    let twice_remainder = remainder.unsigned_abs() * 2;
    let denominator_magnitude = denominator.unsigned_abs();
    let round = twice_remainder > denominator_magnitude
        || (twice_remainder == denominator_magnitude && quotient % 2 != 0);
    if !round {
        return Ok(quotient);
    }
    let direction = if numerator.is_negative() == denominator.is_negative() {
        1
    } else {
        -1
    };
    quotient
        .checked_add(direction)
        .ok_or(NumericError::Overflow)
}

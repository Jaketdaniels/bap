use crate::NumericError;

/// A closed interval with finite or infinite bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    lo: f64,
    hi: f64,
}

impl Interval {
    /// Creates an interval, rejecting NaN and reversed bounds.
    pub fn new(lo: f64, hi: f64) -> Result<Self, NumericError> {
        if lo.is_nan() || hi.is_nan() || lo > hi {
            return Err(NumericError::InvalidArgument);
        }
        Ok(Self { lo, hi })
    }

    pub const fn lo(self) -> f64 {
        self.lo
    }

    pub const fn hi(self) -> f64 {
        self.hi
    }

    pub const fn bounds(self) -> (f64, f64) {
        (self.lo, self.hi)
    }

    pub fn contains(self, value: f64) -> bool {
        !value.is_nan() && self.lo <= value && value <= self.hi
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, NumericError> {
        widened(self.lo + rhs.lo, self.hi + rhs.hi)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, NumericError> {
        widened(self.lo - rhs.hi, self.hi - rhs.lo)
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, NumericError> {
        let values = [
            mul_corner(self.lo, rhs.lo),
            mul_corner(self.lo, rhs.hi),
            mul_corner(self.hi, rhs.lo),
            mul_corner(self.hi, rhs.hi),
        ];
        widened_min_max(values)
    }

    pub fn checked_div(self, rhs: Self) -> Result<Self, NumericError> {
        if rhs.lo <= 0.0 && rhs.hi >= 0.0 {
            return Err(NumericError::Discontinuous);
        }
        let values = [
            self.lo / rhs.lo,
            self.lo / rhs.hi,
            self.hi / rhs.lo,
            self.hi / rhs.hi,
        ];
        widened_min_max(values)
    }
}

fn widened(lo: f64, hi: f64) -> Result<Interval, NumericError> {
    if lo.is_nan() || hi.is_nan() {
        return Err(NumericError::Indeterminate);
    }
    Interval::new(next_down_finite(lo), next_up_finite(hi))
}

fn widened_min_max(values: [f64; 4]) -> Result<Interval, NumericError> {
    if values.iter().any(|value| value.is_nan()) {
        return Err(NumericError::Indeterminate);
    }
    let mut lo = values[0];
    let mut hi = values[0];
    for value in values.into_iter().skip(1) {
        lo = lo.min(value);
        hi = hi.max(value);
    }
    widened(lo, hi)
}

fn mul_corner(lhs: f64, rhs: f64) -> f64 {
    if (lhs == 0.0 && rhs.is_infinite()) || (rhs == 0.0 && lhs.is_infinite()) {
        0.0
    } else {
        lhs * rhs
    }
}

fn next_up_finite(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value > 0.0 { bits + 1 } else { bits - 1 })
}

fn next_down_finite(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits((1_u64 << 63) | 1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value > 0.0 { bits - 1 } else { bits + 1 })
}

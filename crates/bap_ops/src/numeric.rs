//! Audited numeric ops over `bap_math`: intervals, checked ints, and fixed point.

use super::metrics::record_op;
use bap_math::{CheckedI32, CheckedI64, Fixed18_4, Interval, NumericError};
use deno_core::OpState;
use deno_core::op2;
use deno_error::JsErrorBox;
use std::time::Instant;

/// Binop selectors shared with `js/numeric.js`.
const OP_ADD: u8 = 0;
const OP_SUB: u8 = 1;
const OP_MUL: u8 = 2;
const OP_DIV: u8 = 3;

trait CheckedOps: Copy {
    fn add(self, rhs: Self) -> Result<Self, NumericError>;
    fn sub(self, rhs: Self) -> Result<Self, NumericError>;
    fn mul(self, rhs: Self) -> Result<Self, NumericError>;
    fn div(self, rhs: Self) -> Result<Self, NumericError>;
}

impl CheckedOps for CheckedI32 {
    fn add(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_add(rhs)
    }
    fn sub(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_sub(rhs)
    }
    fn mul(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_mul(rhs)
    }
    fn div(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_div(rhs)
    }
}

impl CheckedOps for CheckedI64 {
    fn add(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_add(rhs)
    }
    fn sub(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_sub(rhs)
    }
    fn mul(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_mul(rhs)
    }
    fn div(self, rhs: Self) -> Result<Self, NumericError> {
        self.checked_div(rhs)
    }
}

/// Maps a numeric-domain failure onto `RangeError` carrying its stable code.
fn numeric_err(err: NumericError) -> JsErrorBox {
    JsErrorBox::new("RangeError", err.to_string())
}

fn apply_binop<T: CheckedOps>(opcode: u8, lhs: T, rhs: T) -> Result<T, NumericError> {
    match opcode {
        OP_ADD => lhs.add(rhs),
        OP_SUB => lhs.sub(rhs),
        OP_MUL => lhs.mul(rhs),
        OP_DIV => lhs.div(rhs),
        _ => Err(NumericError::InvalidArgument),
    }
}

fn apply_interval(
    opcode: u8,
    a_lo: f64,
    a_hi: f64,
    b_lo: f64,
    b_hi: f64,
) -> Result<Interval, NumericError> {
    let lhs = Interval::new(a_lo, a_hi)?;
    let rhs = Interval::new(b_lo, b_hi)?;
    match opcode {
        OP_ADD => lhs.checked_add(rhs),
        OP_SUB => lhs.checked_sub(rhs),
        OP_MUL => lhs.checked_mul(rhs),
        OP_DIV => lhs.checked_div(rhs),
        _ => Err(NumericError::InvalidArgument),
    }
}

#[op2]
#[serde]
pub fn op_numeric_interval_new(lo: f64, hi: f64) -> Result<(f64, f64), JsErrorBox> {
    let interval = Interval::new(lo, hi).map_err(numeric_err)?;
    Ok((interval.lo(), interval.hi()))
}

macro_rules! interval_binop {
    ($($name:ident, $opcode:expr, $metric:literal;)*) => {
        $(
            #[op2]
            #[serde]
            pub fn $name(
                state: &mut OpState,
                a_lo: f64,
                a_hi: f64,
                b_lo: f64,
                b_hi: f64,
            ) -> Result<(f64, f64), JsErrorBox> {
                let started = Instant::now();
                let result = apply_interval($opcode, a_lo, a_hi, b_lo, b_hi);
                record_op(state, $metric, started);
                let value = result.map_err(numeric_err)?;
                Ok((value.lo(), value.hi()))
            }
        )*
    };
}

interval_binop!(
    op_numeric_interval_add, OP_ADD, "numeric.interval.add";
    op_numeric_interval_sub, OP_SUB, "numeric.interval.sub";
    op_numeric_interval_mul, OP_MUL, "numeric.interval.mul";
    op_numeric_interval_div, OP_DIV, "numeric.interval.div";
);

macro_rules! interval_batch {
    ($($name:ident, $opcode:expr, $metric:literal;)*) => {
        $(
            /// Computes the whole batch in one crossing; any failure rejects it whole.
            #[op2]
            #[serde]
            pub fn $name(
                state: &mut OpState,
                #[serde] left: Vec<(f64, f64)>,
                #[serde] right: Vec<(f64, f64)>,
            ) -> Result<Vec<(f64, f64)>, JsErrorBox> {
                let started = Instant::now();
                if left.len() != right.len() {
                    return Err(numeric_err(NumericError::InvalidArgument));
                }
                let mut out = Vec::with_capacity(left.len());
                for (&(a_lo, a_hi), &(b_lo, b_hi)) in left.iter().zip(right.iter()) {
                    match apply_interval($opcode, a_lo, a_hi, b_lo, b_hi) {
                        Ok(value) => out.push((value.lo(), value.hi())),
                        Err(err) => return Err(numeric_err(err)),
                    }
                }
                record_op(state, $metric, started);
                Ok(out)
            }
        )*
    };
}

interval_batch!(
    op_numeric_interval_add_batch, OP_ADD, "numeric.batch.interval.add";
    op_numeric_interval_sub_batch, OP_SUB, "numeric.batch.interval.sub";
    op_numeric_interval_mul_batch, OP_MUL, "numeric.batch.interval.mul";
    op_numeric_interval_div_batch, OP_DIV, "numeric.batch.interval.div";
);

#[op2(fast)]
pub fn op_numeric_i32_binop(
    state: &mut OpState,
    opcode: u32,
    lhs: i32,
    rhs: i32,
) -> Result<i32, JsErrorBox> {
    let started = Instant::now();
    let result = apply_binop(opcode as u8, CheckedI32::new(lhs), CheckedI32::new(rhs));
    record_op(state, "numeric.i32.binop", started);
    result.map(CheckedI32::get).map_err(numeric_err)
}

#[op2(fast)]
pub fn op_numeric_i32_neg(state: &mut OpState, value: i32) -> Result<i32, JsErrorBox> {
    let started = Instant::now();
    let result = CheckedI32::new(value).checked_neg();
    record_op(state, "numeric.i32.neg", started);
    result.map(CheckedI32::get).map_err(numeric_err)
}

#[op2(fast)]
#[bigint]
pub fn op_numeric_i64_binop(
    state: &mut OpState,
    opcode: u32,
    #[bigint] lhs: i64,
    #[bigint] rhs: i64,
) -> Result<i64, JsErrorBox> {
    let started = Instant::now();
    let result = apply_binop(opcode as u8, CheckedI64::new(lhs), CheckedI64::new(rhs));
    record_op(state, "numeric.i64.binop", started);
    result.map(CheckedI64::get).map_err(numeric_err)
}

#[op2(fast)]
#[bigint]
pub fn op_numeric_i64_neg(state: &mut OpState, #[bigint] value: i64) -> Result<i64, JsErrorBox> {
    let started = Instant::now();
    let result = CheckedI64::new(value).checked_neg();
    record_op(state, "numeric.i64.neg", started);
    result.map(CheckedI64::get).map_err(numeric_err)
}

/// Adds two equal-length arrays atomically; overflow rejects the whole batch.
#[op2]
#[serde]
pub fn op_numeric_i64_add_batch(
    state: &mut OpState,
    #[serde] left: Vec<i64>,
    #[serde] right: Vec<i64>,
) -> Result<Vec<i64>, JsErrorBox> {
    let started = Instant::now();
    if left.len() != right.len() {
        return Err(numeric_err(NumericError::InvalidArgument));
    }
    let mut out = Vec::with_capacity(left.len());
    for (&lhs, &rhs) in left.iter().zip(right.iter()) {
        match CheckedI64::new(lhs).checked_add(CheckedI64::new(rhs)) {
            Ok(value) => out.push(value.get()),
            Err(err) => return Err(numeric_err(err)),
        }
    }
    record_op(state, "numeric.batch.i64.add", started);
    Ok(out)
}

/// Validates and canonicalizes a decimal literal.
#[op2]
#[string]
pub fn op_numeric_fixed_parse(
    state: &mut OpState,
    #[string] text: String,
) -> Result<String, JsErrorBox> {
    let started = Instant::now();
    let value: Fixed18_4 = text.parse().map_err(numeric_err)?;
    record_op(state, "numeric.fixed.parse", started);
    Ok(value.to_string())
}

/// Applies one fixed-point operation to two decimal literals.
#[op2]
#[string]
pub fn op_numeric_fixed_binop(
    state: &mut OpState,
    opcode: u32,
    #[string] lhs: String,
    #[string] rhs: String,
) -> Result<String, JsErrorBox> {
    let started = Instant::now();
    let left: Fixed18_4 = lhs.parse().map_err(numeric_err)?;
    let right: Fixed18_4 = rhs.parse().map_err(numeric_err)?;
    let result = match opcode as u8 {
        OP_ADD => left.checked_add(right),
        OP_SUB => left.checked_sub(right),
        OP_MUL => left.checked_mul(right),
        OP_DIV => left.checked_div(right),
        _ => Err(NumericError::InvalidArgument),
    };
    let value = result.map_err(numeric_err)?;
    record_op(state, "numeric.fixed.binop", started);
    Ok(value.to_string())
}

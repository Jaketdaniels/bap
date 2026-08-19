use bap_math::{CheckedI32, CheckedI64, NumericError};
use proptest::prelude::*;

#[test]
fn checked_integer_boundaries_never_wrap() {
    assert_eq!(
        CheckedI32::new(i32::MAX).checked_add(CheckedI32::new(1)),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        CheckedI32::new(i32::MIN).checked_sub(CheckedI32::new(1)),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        CheckedI64::new(i64::MAX).checked_mul(CheckedI64::new(2)),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        CheckedI64::new(i64::MIN).checked_neg(),
        Err(NumericError::Overflow)
    );
}

#[test]
fn checked_integer_division_distinguishes_zero_from_overflow() {
    assert_eq!(
        CheckedI32::new(1).checked_div(CheckedI32::new(0)),
        Err(NumericError::DivisionByZero)
    );
    assert_eq!(
        CheckedI32::new(i32::MIN).checked_div(CheckedI32::new(-1)),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        CheckedI64::new(1).checked_div(CheckedI64::new(0)),
        Err(NumericError::DivisionByZero)
    );
    assert_eq!(
        CheckedI64::new(i64::MIN).checked_div(CheckedI64::new(-1)),
        Err(NumericError::Overflow)
    );
}

#[test]
fn checked_integer_new_and_get_preserve_values() {
    assert_eq!(CheckedI32::new(-42).get(), -42);
    assert_eq!(
        CheckedI64::new(9_007_199_254_740_993).get(),
        9_007_199_254_740_993
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn checked_i32_add_matches_the_standard_library(a in any::<i32>(), b in any::<i32>()) {
        let actual = CheckedI32::new(a).checked_add(CheckedI32::new(b));
        let expected = a.checked_add(b).map(CheckedI32::new).ok_or(NumericError::Overflow);
        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn checked_i64_arithmetic_matches_the_standard_library(a in any::<i64>(), b in any::<i64>()) {
        let lhs = CheckedI64::new(a);
        let rhs = CheckedI64::new(b);
        prop_assert_eq!(lhs.checked_add(rhs), a.checked_add(b).map(CheckedI64::new).ok_or(NumericError::Overflow));
        prop_assert_eq!(lhs.checked_sub(rhs), a.checked_sub(b).map(CheckedI64::new).ok_or(NumericError::Overflow));
        prop_assert_eq!(lhs.checked_mul(rhs), a.checked_mul(b).map(CheckedI64::new).ok_or(NumericError::Overflow));
    }
}

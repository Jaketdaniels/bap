use bap_math::{Interval, NumericError};
use num_bigint::BigInt;
use num_rational::BigRational;
use proptest::prelude::*;

#[test]
fn interval_constructor_accepts_infinity_and_rejects_invalid_bounds() {
    assert_eq!(
        Interval::new(f64::NEG_INFINITY, f64::INFINITY)
            .unwrap()
            .bounds(),
        (f64::NEG_INFINITY, f64::INFINITY)
    );
    assert_eq!(
        Interval::new(f64::NAN, 1.0),
        Err(NumericError::InvalidArgument)
    );
    assert_eq!(Interval::new(2.0, 1.0), Err(NumericError::InvalidArgument));
}

#[test]
fn interval_errors_have_stable_codes() {
    assert_eq!(
        NumericError::InvalidArgument.code(),
        "BAP_NUMERIC_INVALID_ARGUMENT"
    );
    assert_eq!(
        NumericError::Indeterminate.code(),
        "BAP_NUMERIC_INDETERMINATE"
    );
    assert_eq!(
        NumericError::Discontinuous.code(),
        "BAP_NUMERIC_DISCONTINUOUS"
    );
}

#[test]
fn interval_addition_rejects_opposing_infinities() {
    let positive = Interval::new(f64::INFINITY, f64::INFINITY).unwrap();
    let negative = Interval::new(f64::NEG_INFINITY, f64::NEG_INFINITY).unwrap();
    assert_eq!(
        positive.checked_add(negative),
        Err(NumericError::Indeterminate)
    );
    assert_eq!(
        positive.checked_sub(positive),
        Err(NumericError::Indeterminate)
    );
}

#[test]
fn interval_multiplication_defines_zero_times_infinity_as_zero() {
    let zero = Interval::new(0.0, 0.0).unwrap();
    let unbounded = Interval::new(f64::NEG_INFINITY, f64::INFINITY).unwrap();
    let result = zero.checked_mul(unbounded).unwrap();
    assert!(result.contains(0.0));
    assert!(result.lo() <= 0.0);
    assert!(result.hi() >= 0.0);
}

#[test]
fn interval_division_rejects_every_zero_spanning_interval() {
    let numerator = Interval::new(1.0, 2.0).unwrap();
    for zero in [
        Interval::new(-1.0, 1.0).unwrap(),
        Interval::new(-0.0, 1.0).unwrap(),
        Interval::new(-1.0, -0.0).unwrap(),
        Interval::new(0.0, 0.0).unwrap(),
    ] {
        assert_eq!(
            numerator.checked_div(zero),
            Err(NumericError::Discontinuous)
        );
    }
}

#[test]
fn interval_rounding_contains_subnormal_and_cancellation_results() {
    let smallest = f64::from_bits(1);
    let subnormal = Interval::new(smallest, smallest)
        .unwrap()
        .checked_add(Interval::new(smallest, smallest).unwrap())
        .unwrap();
    assert!(subnormal.contains(smallest + smallest));

    let cancellation = Interval::new(1.0, 1.0)
        .unwrap()
        .checked_sub(Interval::new(1.0, 1.0).unwrap())
        .unwrap();
    assert!(cancellation.lo().is_sign_negative());
    assert!(cancellation.contains(-0.0));
    assert!(cancellation.contains(0.0));
}

#[test]
fn interval_overflow_remains_infinite() {
    let max = Interval::new(f64::MAX, f64::MAX).unwrap();
    let result = max.checked_add(max).unwrap();
    assert_eq!(result.lo(), f64::INFINITY);
    assert_eq!(result.hi(), f64::INFINITY);
}

fn finite_interval() -> impl Strategy<Value = Interval> {
    (-1.0e100_f64..1.0e100, -1.0e100_f64..1.0e100)
        .prop_map(|(a, b)| Interval::new(a.min(b), a.max(b)).unwrap())
}

fn positive_interval() -> impl Strategy<Value = Interval> {
    (1.0e-100_f64..1.0e100, 1.0e-100_f64..1.0e100)
        .prop_map(|(a, b)| Interval::new(a.min(b), a.max(b)).unwrap())
}

fn exact(value: f64) -> BigRational {
    assert!(value.is_finite());
    if value == 0.0 {
        return BigRational::from_integer(BigInt::from(0));
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let raw_exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if raw_exponent == 0 {
        (fraction, -1022 - 52)
    } else {
        ((1_u64 << 52) | fraction, raw_exponent - 1023 - 52)
    };
    let mut numerator = BigInt::from(significand);
    if negative {
        numerator = -numerator;
    }
    if exponent >= 0 {
        BigRational::from_integer(numerator << exponent as usize)
    } else {
        BigRational::new(numerator, BigInt::from(1_u8) << (-exponent) as usize)
    }
}

fn contains_exact(interval: Interval, value: BigRational) -> bool {
    exact(interval.lo()) <= value && value <= exact(interval.hi())
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn interval_addition_contains_endpoint_results(a in finite_interval(), b in finite_interval()) {
        let result = a.checked_add(b).unwrap();
        prop_assert!(contains_exact(result, exact(a.lo()) + exact(b.lo())));
        prop_assert!(contains_exact(result, exact(a.hi()) + exact(b.hi())));
    }

    #[test]
    fn interval_subtraction_contains_endpoint_results(a in finite_interval(), b in finite_interval()) {
        let result = a.checked_sub(b).unwrap();
        prop_assert!(contains_exact(result, exact(a.lo()) - exact(b.hi())));
        prop_assert!(contains_exact(result, exact(a.hi()) - exact(b.lo())));
    }

    #[test]
    fn interval_multiplication_contains_all_corner_results(a in finite_interval(), b in finite_interval()) {
        let result = a.checked_mul(b).unwrap();
        for value in [
            exact(a.lo()) * exact(b.lo()),
            exact(a.lo()) * exact(b.hi()),
            exact(a.hi()) * exact(b.lo()),
            exact(a.hi()) * exact(b.hi()),
        ] {
            prop_assert!(contains_exact(result, value));
        }
    }

    #[test]
    fn interval_division_contains_all_corner_results(a in finite_interval(), b in positive_interval()) {
        let result = a.checked_div(b).unwrap();
        for value in [
            exact(a.lo()) / exact(b.lo()),
            exact(a.lo()) / exact(b.hi()),
            exact(a.hi()) / exact(b.lo()),
            exact(a.hi()) / exact(b.hi()),
        ] {
            prop_assert!(contains_exact(result, value));
        }
    }
}

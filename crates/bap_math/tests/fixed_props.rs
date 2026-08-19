use bap_math::{Fixed, Fixed18_4, NumericError};
use proptest::prelude::*;

#[test]
fn fixed_18_4_uses_total_precision_and_half_even() {
    assert_eq!(
        "99999999999999.9999"
            .parse::<Fixed18_4>()
            .unwrap()
            .to_string(),
        "99999999999999.9999"
    );
    assert!("100000000000000.0000".parse::<Fixed18_4>().is_err());
    assert_eq!(
        "1.23445".parse::<Fixed18_4>().unwrap().to_string(),
        "1.2344"
    );
    assert_eq!(
        "1.23455".parse::<Fixed18_4>().unwrap().to_string(),
        "1.2346"
    );
    assert_eq!(
        "-1.23445".parse::<Fixed18_4>().unwrap().to_string(),
        "-1.2344"
    );
    assert_eq!(
        "-1.23455".parse::<Fixed18_4>().unwrap().to_string(),
        "-1.2346"
    );
}

#[test]
fn fixed_rejects_invalid_precision_and_non_decimal_syntax() {
    assert_eq!(
        "1".parse::<Fixed<0, 0>>(),
        Err(NumericError::InvalidConfiguration)
    );
    assert_eq!(
        "1".parse::<Fixed<2, 3>>(),
        Err(NumericError::InvalidConfiguration)
    );
    for input in [
        "", " ", " 1", "1 ", ".5", "1.", "+", "-", "1e2", "1_000", "--1", "1.2.3",
    ] {
        assert_eq!(
            input.parse::<Fixed18_4>(),
            Err(NumericError::InvalidArgument),
            "{input:?}"
        );
    }
}

#[test]
fn fixed_rounding_enforces_limits_after_rounding() {
    assert_eq!(
        "99999999999999.99994"
            .parse::<Fixed18_4>()
            .unwrap()
            .coefficient(),
        999_999_999_999_999_999
    );
    assert_eq!(
        "99999999999999.99995".parse::<Fixed18_4>(),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        "-99999999999999.99995".parse::<Fixed18_4>(),
        Err(NumericError::Overflow)
    );
}

#[test]
fn fixed_display_is_canonical() {
    assert_eq!("+001.2".parse::<Fixed18_4>().unwrap().to_string(), "1.2000");
    assert_eq!(
        "-0.00001".parse::<Fixed18_4>().unwrap().to_string(),
        "0.0000"
    );
    assert_eq!("42".parse::<Fixed<2, 0>>().unwrap().to_string(), "42");
}

#[test]
fn fixed_new_checks_coefficient_range() {
    assert_eq!(Fixed18_4::new(7).unwrap().coefficient(), 7);
    assert_eq!(
        Fixed18_4::new(1_000_000_000_000_000_000),
        Err(NumericError::Overflow)
    );
}

#[test]
fn fixed_checked_add_and_sub_reject_overflow() {
    let max = "99999999999999.9999".parse::<Fixed18_4>().unwrap();
    let min = "-99999999999999.9999".parse::<Fixed18_4>().unwrap();
    let unit = "0.0001".parse::<Fixed18_4>().unwrap();
    assert_eq!(max.checked_add(unit), Err(NumericError::Overflow));
    assert_eq!(min.checked_sub(unit), Err(NumericError::Overflow));
}

#[test]
fn fixed_multiplication_and_division_round_half_even() {
    type Money = Fixed<4, 2>;
    let tenth = "0.10".parse::<Money>().unwrap();
    assert_eq!(
        "0.25"
            .parse::<Money>()
            .unwrap()
            .checked_mul(tenth)
            .unwrap()
            .to_string(),
        "0.02"
    );
    assert_eq!(
        "0.35"
            .parse::<Money>()
            .unwrap()
            .checked_mul(tenth)
            .unwrap()
            .to_string(),
        "0.04"
    );
    let eight = "8.00".parse::<Money>().unwrap();
    assert_eq!(
        "1.00"
            .parse::<Money>()
            .unwrap()
            .checked_div(eight)
            .unwrap()
            .to_string(),
        "0.12"
    );
    assert_eq!(
        "3.00"
            .parse::<Money>()
            .unwrap()
            .checked_div(eight)
            .unwrap()
            .to_string(),
        "0.38"
    );
}

#[test]
fn fixed_division_reports_zero_separately() {
    type Money = Fixed<4, 2>;
    assert_eq!(
        "1.00"
            .parse::<Money>()
            .unwrap()
            .checked_div("0.00".parse().unwrap()),
        Err(NumericError::DivisionByZero)
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn fixed_addition_matches_exact_coefficients(a in -499_999_i128..=499_999, b in -499_999_i128..=499_999) {
        type TestFixed = Fixed<6, 2>;
        let lhs = TestFixed::new(a).unwrap();
        let rhs = TestFixed::new(b).unwrap();
        let sum = a + b;
        let actual = lhs.checked_add(rhs);
        if sum.unsigned_abs() <= 999_999 {
            prop_assert_eq!(actual.unwrap().coefficient(), sum);
        } else {
            prop_assert_eq!(actual, Err(NumericError::Overflow));
        }
    }
}

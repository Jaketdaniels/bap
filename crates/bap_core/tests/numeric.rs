//! Integration tests for the `bap.numeric` surface through a live runtime.

use bap_core::BapRuntime;
use bap_metrics::MetricsCollector;
use serde_json::Value;

fn runtime() -> BapRuntime {
    BapRuntime::new(MetricsCollector::new())
}

fn eval_value(code: &str) -> Value {
    let mut rt = runtime();
    rt.eval(code)
        .expect("numeric expression should evaluate")
        .expect("expression should produce a value")
}

fn eval_error(code: &str) -> String {
    let mut rt = runtime();
    rt.eval(code)
        .expect_err("expression should throw")
        .to_string()
}

#[test]
fn interval_new_reports_validated_bounds() {
    let value = eval_value("bap.numeric.interval(1, 2)");
    assert_eq!(value["lo"].as_f64().unwrap(), 1.0);
    assert_eq!(value["hi"].as_f64().unwrap(), 2.0);
}

#[test]
fn interval_add_keeps_outward_rounding() {
    let value = eval_value(
        "const s = bap.numeric.intervalAdd(bap.numeric.interval(1, 2), bap.numeric.interval(10, 20));
         JSON.stringify({lo: s.lo, hi: s.hi})",
    );
    let parsed: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    let lo = parsed["lo"].as_f64().unwrap();
    let hi = parsed["hi"].as_f64().unwrap();
    assert!(lo < 11.0 && lo > 10.9999999999, "lo={lo}");
    assert!(hi > 22.0 && hi < 22.0000000001, "hi={hi}");
}

#[test]
fn interval_division_rejects_zero_spanning_divisor() {
    let err = eval_error(
        "bap.numeric.intervalDiv(bap.numeric.interval(1, 2), bap.numeric.interval(-1, 1))",
    );
    assert!(err.contains("BAP_NUMERIC_DISCONTINUOUS"), "was: {err}");
}

#[test]
fn forged_interval_is_rejected() {
    let err = eval_error("bap.numeric.intervalAdd({lo: 0, hi: 1}, bap.numeric.interval(1, 2))");
    assert!(err.contains("BAP_NUMERIC_INVALID_ARGUMENT"), "was: {err}");
    assert!(err.contains("TypeError"), "was: {err}");
}

#[test]
fn non_numeric_arguments_are_type_errors() {
    let err = eval_error("bap.numeric.interval('a', 'b')");
    assert!(err.contains("BAP_NUMERIC_INVALID_ARGUMENT"), "was: {err}");
}

#[test]
fn intervals_and_results_are_frozen() {
    let value = eval_value(
        "JSON.stringify({
            interval: Object.isFrozen(bap.numeric.interval(0, 1)),
            namespace: Object.isFrozen(bap.numeric),
        })",
    );
    let parsed: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(parsed["interval"], serde_json::json!(true));
    assert_eq!(parsed["namespace"], serde_json::json!(true));
}

#[test]
fn batch_records_one_op_and_freezes_output() {
    let metrics = MetricsCollector::new();
    let mut rt = BapRuntime::new(metrics.clone());
    let code = r#"
        const left = [bap.numeric.interval(0, 1), bap.numeric.interval(2, 3)];
        const right = [bap.numeric.interval(10, 10), bap.numeric.interval(0, 4)];
        const out = bap.numeric.intervalAddBatch(left, right);
        JSON.stringify({
            length: out.length,
            firstLo: out[0].lo,
            secondHi: out[1].hi,
            frozen: Object.isFrozen(out),
        })
    "#;
    let value = rt.eval(code).unwrap().unwrap();
    let parsed: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(parsed["length"], serde_json::json!(2));
    let first_lo = parsed["firstLo"].as_f64().unwrap();
    let second_hi = parsed["secondHi"].as_f64().unwrap();
    assert!(
        first_lo < 10.0 && first_lo > 9.9999999999,
        "firstLo={first_lo}"
    );
    assert!(
        second_hi > 7.0 && second_hi < 7.0000000001,
        "secondHi={second_hi}"
    );
    assert_eq!(parsed["frozen"], serde_json::json!(true));
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.ops["numeric.batch.interval.add"].calls, 1);
}

#[test]
fn batch_length_mismatch_is_rejected() {
    let err = eval_error("bap.numeric.intervalAddBatch([bap.numeric.interval(0, 1)], [])");
    assert!(err.contains("BAP_NUMERIC_INVALID_ARGUMENT"), "was: {err}");
}

#[test]
fn batch_interval_overflow_rejects_atomically() {
    let err = eval_error(
        "bap.numeric.intervalDivBatch(
            [bap.numeric.interval(1, 2)],
            [bap.numeric.interval(0, 0)]
        )",
    );
    assert!(err.contains("BAP_NUMERIC_DISCONTINUOUS"), "was: {err}");
}

#[test]
fn i64_roundtrip_stays_exact() {
    let value = eval_value(
        "const max = 9223372036854775807n;
         (bap.numeric.i64Add(max - 2n, 2n) === max) +
         (bap.numeric.i64Mul(3037000499n, 3037000499n) === 9223372030926249001n)",
    );
    assert_eq!(value, serde_json::json!(2));
}

#[test]
fn i64_overflow_throws_range_error() {
    let err = eval_error("bap.numeric.i64Add(9223372036854775807n, 1n)");
    assert!(err.contains("BAP_NUMERIC_OVERFLOW"), "was: {err}");
}

#[test]
fn i64_batch_rejects_on_any_overflow() {
    let err = eval_error("bap.numeric.i64AddBatch([1n, 9223372036854775807n], [2n, 1n])");
    assert!(err.contains("BAP_NUMERIC_OVERFLOW"), "was: {err}");
}

#[test]
fn i32_ops_wrap_never() {
    let value = eval_value(
        "(bap.numeric.i32Add(2147483646, 1) === 2147483647) + (() => { try { bap.numeric.i32Add(2147483647, 1); return 0; } catch { return 1; } })()",
    );
    assert_eq!(value, serde_json::json!(2));
}

#[test]
fn fixed_parses_canonically_and_rounds_half_even() {
    let value = eval_value(
        "JSON.stringify([
            bap.numeric.fixed('12.3456'),
            bap.numeric.fixed('1.23445'),
            bap.numeric.fixed('-0.00005')
        ])",
    );
    let parsed: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(parsed, serde_json::json!(["12.3456", "1.2344", "0.0000"]));
}

#[test]
fn fixed_arithmetic_and_errors_carry_codes() {
    let value = eval_value(
        "JSON.stringify([
            bap.numeric.fixedAdd('1.5', '2.25'),
            bap.numeric.fixedMul('2.5', '0.4'),
            bap.numeric.fixedDiv('1', '3')
        ])",
    );
    let parsed: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(parsed[0], serde_json::json!("3.7500"));
    assert_eq!(parsed[1], serde_json::json!("1.0000"));
    assert_eq!(parsed[2], serde_json::json!("0.3333"));

    let err = eval_error("bap.numeric.fixedDiv('1', '0')");
    assert!(err.contains("BAP_NUMERIC_DIVISION_BY_ZERO"), "was: {err}");
    let err = eval_error("bap.numeric.fixed('100000000000000.0000')");
    assert!(err.contains("BAP_NUMERIC_OVERFLOW"), "was: {err}");
}

#[test]
fn unit_helpers_enforce_shared_units() {
    let value = eval_value(
        r#"
        const metre = (v) => bap.numeric.unit('metre', v);
        const total = bap.numeric.unitAdd(metre(7), metre(8));
        JSON.stringify({ unit: total.__bapUnit, lo: total.lo, hi: total.hi })
    "#,
    );
    let parsed: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert_eq!(parsed["unit"], serde_json::json!("metre"));
    let lo = parsed["lo"].as_f64().unwrap();
    let hi = parsed["hi"].as_f64().unwrap();
    assert!(lo < 15.0 && hi > 15.0, "unit total [{lo}, {hi}]");

    let err = eval_error(
        "bap.numeric.unitAdd(bap.numeric.unit('metre', 1), bap.numeric.unit('second', 2))",
    );
    assert!(err.contains("BAP_NUMERIC_INVALID_ARGUMENT"), "was: {err}");

    let err = eval_error(
        "bap.numeric.unitMul(bap.numeric.unit('metre', 1), bap.numeric.unit('metre', 2))",
    );
    assert!(err.contains("BAP_NUMERIC_INVALID_ARGUMENT"), "was: {err}");
}

#[test]
fn unit_helpers_reject_forged_values() {
    let forged = r#"({lo: 1, hi: 1, __bapUnit: "metre"})"#;
    let err = eval_error(&format!(
        "bap.numeric.unitAdd({forged}, bap.numeric.unit('metre', 2))"
    ));
    assert!(err.contains("BAP_NUMERIC_INVALID_ARGUMENT"), "was: {err}");
}

#[test]
fn eval_surfaces_bigints_as_literals() {
    let value = eval_value("bap.numeric.i64Add(1n, 2n)");
    assert_eq!(value, serde_json::json!("3n"));
    let big = eval_value("2n ** 100n");
    assert_eq!(big, serde_json::json!("1267650600228229401496703205376n"));
}

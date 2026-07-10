use crate::math::{floor_div_i64, modulo_i64};

#[test]
fn floor_division_rejects_zero_and_overflow() {
    assert_eq!(floor_div_i64(7, 0), None);
    assert_eq!(floor_div_i64(i64::MIN, -1), None);
}

#[test]
fn modulo_rejects_zero_and_overflow() {
    assert_eq!(modulo_i64(7, 0), None);
    assert_eq!(modulo_i64(i64::MIN, -1), None);
}

#[test]
fn euclidean_integer_ops_round_toward_negative_infinity() {
    assert_eq!(floor_div_i64(-7, 3), Some(-3));
    assert_eq!(modulo_i64(-7, 3), Some(2));
}

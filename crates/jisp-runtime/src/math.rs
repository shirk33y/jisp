pub fn floor_div_i64(left: i64, right: i64) -> Option<i64> {
    left.checked_div_euclid(right)
}

pub fn modulo_i64(left: i64, right: i64) -> Option<i64> {
    left.checked_rem_euclid(right)
}

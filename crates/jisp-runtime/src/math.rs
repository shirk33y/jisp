pub fn floor_div_i64(left: i64, right: i64) -> Option<i64> {
    (right != 0).then(|| left.div_euclid(right))
}

pub fn modulo_i64(left: i64, right: i64) -> Option<i64> {
    (right != 0).then(|| left.rem_euclid(right))
}

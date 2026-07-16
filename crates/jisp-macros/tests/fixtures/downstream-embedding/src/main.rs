jisp_macros::lisp_file!("src/report.lisp");

fn main() {
    assert_eq!(report().to_string(), "9223372036854775810");

    let answer: i64 = jisp_macros::lisp_expr!("src/expression.lisp");
    assert_eq!(answer, 42);
}

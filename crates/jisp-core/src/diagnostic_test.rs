use super::*;
use crate::{SourceMap, Span};

#[test]
fn renders_primary_secondary_and_notes() {
    let mut sources = SourceMap::default();
    let source = sources.add("main.lisp", "(def x 1)\n(def y x)\n");
    let imported = sources.add("math.lisp", "(export z 2)\n");
    let diagnostic = Diagnostic::error(Span::new(source, 5, 6), "bad definition")
        .with_code("J0001")
        .with_primary_message("defined here")
        .with_secondary(Span::new(source, 17, 18), "used here")
        .with_secondary(Span::new(imported, 8, 9), "related export")
        .with_note("names must be unique");

    assert_eq!(
        diagnostic.render(&sources),
        "error[J0001]: bad definition\n  --> main.lisp:1:6\n   |\n  1 | (def x 1)\n   |      ^ defined here\n  2 | (def y x)\n   |        - used here\n  --> math.lisp:1:9\n   |\n  1 | (export z 2)\n   |         - related export\n   = note: names must be unique"
    );
}

#[test]
fn renders_multiline_labels() {
    let mut sources = SourceMap::default();
    let source = sources.add("main.lisp", "(def x\n  1)\n");
    let diagnostic = Diagnostic::error(Span::new(source, 0, 10), "multi-line form");

    assert_eq!(
        diagnostic.render(&sources),
        "error: multi-line form\n  --> main.lisp:1:1\n   |\n  1 | (def x\n   | ^^^^^^\n  2 |   1)\n   | ^^^"
    );
}

#[test]
fn renders_multiline_secondary_labels() {
    let mut sources = SourceMap::default();
    let source = sources.add("main.lisp", "(def x 1)\n(def y\n  x)\n");
    let imported = sources.add("math.lisp", "(export\n  z 2)\n");
    let diagnostic = Diagnostic::error(Span::new(source, 5, 6), "bad definition")
        .with_secondary(Span::new(imported, 0, 11), "expanded here");

    assert_eq!(
        diagnostic.render(&sources),
        "error: bad definition\n  --> main.lisp:1:6\n   |\n  1 | (def x 1)\n   |      ^\n  --> math.lisp:1:1\n   |\n  1 | (export\n   | ------- expanded here\n  2 |   z 2)\n   | ---"
    );
}

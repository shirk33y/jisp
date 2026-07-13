use super::render_html_source;

#[test]
fn renders_a_component_program_with_the_real_interpreter() {
    let html = render_html_source(
        r#"
(component todo-row (title)
  (li (class "rounded") (text title)))

(component app ()
  (ul
    (for title (list "Plan" "Ship")
      (todo-row title))))

(export main
  (fn ()
    (ui.html (app))))
"#,
    )
    .unwrap();

    assert_eq!(
        html,
        "<ul><li class=\"rounded\">Plan</li><li class=\"rounded\">Ship</li></ul>"
    );
}

#[test]
fn all_playground_examples_are_valid_interpreter_programs() {
    let examples = [
        (
            "welcome",
            include_str!("../../../playground/examples/welcome.lisp"),
        ),
        (
            "todos",
            include_str!("../../../playground/examples/todos.lisp"),
        ),
        (
            "profile",
            include_str!("../../../playground/examples/profile.lisp"),
        ),
        (
            "notifications",
            include_str!("../../../playground/examples/notifications.lisp"),
        ),
        (
            "dashboard",
            include_str!("../../../playground/examples/dashboard.lisp"),
        ),
        (
            "settings",
            include_str!("../../../playground/examples/settings.lisp"),
        ),
        (
            "product",
            include_str!("../../../playground/examples/product.lisp"),
        ),
        (
            "navigation",
            include_str!("../../../playground/examples/navigation.lisp"),
        ),
        (
            "empty state",
            include_str!("../../../playground/examples/empty-state.lisp"),
        ),
        (
            "projects",
            include_str!("../../../playground/examples/projects.lisp"),
        ),
    ];

    for (name, source) in examples {
        let html = render_html_source(source)
            .unwrap_or_else(|error| panic!("playground example `{name}` did not render: {error}"));
        assert!(
            html.starts_with('<'),
            "playground example `{name}` returned {html:?}"
        );
    }
}

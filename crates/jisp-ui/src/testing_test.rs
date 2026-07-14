use jisp_core::{SourceId, SyntaxParser};
use jisp_syntax_lisp::LispParser;

use crate::testing::{run_ui_tests, split_ui_tests};

fn suite(source: &str) -> crate::testing::UiTestSuite {
    split_ui_tests(LispParser.parse_module(SourceId(0), source).unwrap()).unwrap()
}

#[test]
fn ui_tests_cover_state_html_and_juir_tree_after_dispatches() {
    let outcomes = run_ui_tests(suite(
        r#"
(type Action (Increment))
(def init 0)
(defn update (state action)
  (case action
    ((Increment) (+ state 1))))
(component app (state)
  (button (text (str.from state))))
(ui.app init update app)

(ui.test "counter increments"
  (assert (= "<button>0</button>" (ui.test.html)))
  (assert (= 0 (ui.test.state)))
  (dispatch Increment)
  (assert (= 1 (ui.test.state)))
  (assert (= "<button>1</button>" (ui.test.html))))
"#,
    ))
    .unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].passed(), "{:?}", outcomes[0]);
    assert_eq!(outcomes[0].assertions, 4);
}

#[test]
fn ui_tests_observe_reducer_declared_resources_without_running_them() {
    let outcomes = run_ui_tests(suite(
        r#"
(type Action (Save))
(def init 0)
(defn update (state action)
  (ui.result
    state
    (list
      (ui.command "save:1" "storage.write" 1 (obj "key" "draft") true
        (ui.action-result "Saved" (list))
        (ui.action-error "SaveFailed" (list))))
    (list)))
(component app (state) (button (text (str.from state))))
(ui.app init update app)
(ui.test "declares save resource"
  (assert (= (list) (ui.test.commands)))
  (assert (= (list) (ui.test.subscriptions)))
  (dispatch Save)
  (assert (= (list (ui.command "save:1" "storage.write" 1 (obj "key" "draft") true (ui.action-result "Saved" (list)) (ui.action-error "SaveFailed" (list))) (ui.test.commands)))
  (assert (= (list) (ui.test.subscriptions))))
"#,
    ))
    .unwrap();

    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].passed(), "{:?}", outcomes[0].failure);
    assert_eq!(outcomes[0].assertions, 4);
}

#[test]
fn ui_tests_report_failing_expectations_without_hiding_other_scenarios() {
    let outcomes = run_ui_tests(suite(
        r#"
(type Action (Increment))
(def init 0)
(defn update (state action)
  (case action
    ((Increment) (+ state 1))))
(component app (state) (button (text (str.from state))))
(ui.app init update app)
(ui.test "fails" (assert (= 2 (ui.test.state))))
(ui.test "still runs" (assert (= 0 (ui.test.state))))
"#,
    ))
    .unwrap();
    assert!(!outcomes[0].passed());
    assert!(outcomes[1].passed(), "{:?}", outcomes[1]);
}

#[test]
fn ui_test_compares_event_handler_shape_without_comparing_closures() {
    let outcomes = run_ui_tests(suite(
        r#"
(type Action (Increment))
(def init 0)
(defn update (state action) (+ state 1))
(component app (state)
  (button (on click (emit Increment)) (text (str.from state))))
(ui.app init update app)
(ui.test "eventful button" (assert (= 0 (ui.test.state))))
"#,
    ))
    .unwrap();
    assert!(outcomes[0].passed(), "{:?}", outcomes[0]);
}

#[test]
fn ui_test_requires_one_observable_accessor_in_an_equality_assertion() {
    let error = split_ui_tests(
        LispParser
            .parse_module(SourceId(0), r#"(ui.test "bad" (assert (= 1 1)))"#)
            .unwrap(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("exactly one ui.test accessor"));
}

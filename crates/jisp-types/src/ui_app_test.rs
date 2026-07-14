use jisp_core::{SourceId, SyntaxParser};
use jisp_ir::Lowerer;
use jisp_syntax_lisp::LispParser;

use super::Inferencer;

fn infer(source: &str) -> Result<(), String> {
    let nodes = LispParser
        .parse_module(SourceId(0), source)
        .map_err(|error| error.to_string())?;
    let module = Lowerer
        .lower_module(&nodes)
        .map_err(|error| error.to_string())?;
    Inferencer::with_prelude()
        .infer_typed_module(module)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

const VIEW: &str = r#"
(component app (state)
  (div (text (str.from state))))
"#;

#[test]
fn ui_app_accepts_plain_state_and_explicit_update_result() {
    let plain = format!(
        r#"
(def init 0)
(defn update (state action) (+ state 1))
{VIEW}
(ui.app init update app)
"#
    );
    infer(&plain).unwrap();

    let explicit = format!(
        r#"
(def init 0)
(defn update (state action)
  (ui.result (+ state 1) (list) (list)))
{VIEW}
(ui.app init update app)
"#
    );
    infer(&explicit).unwrap();

    let resources = format!(
        r#"
(def init 0)
(defn update (state action)
  (ui.result
    (+ state 1)
    (list (ui.command "save:1" "storage.write" 1 (obj "key" "draft") true (ui.action "Saved" (list)) (ui.action-error "SaveFailed" (list))))
    (list (ui.subscription "clock" "timer.tick" 1 (obj "every-ms" 1000) false (ui.action-result "Tick" (list)) (ui.action-error "ClockFailed" (list))))))
{VIEW}
(ui.app init update app)
"#
    );
    infer(&resources).unwrap();
}

#[test]
fn ui_app_rejects_reducer_or_view_that_breaks_its_contract() {
    let bad_update = format!(
        r#"
(def init 0)
(defn update (state action) "wrong")
{VIEW}
(ui.app init update app)
"#
    );
    assert!(infer(&bad_update)
        .unwrap_err()
        .contains("update must return state or ui.update-result(state)"));

    let bad_view = r#"
(def init 0)
(defn update (state action) state)
(component app (state) (ui.result state (list) (list)))
(ui.app init update app)
"#;
    assert!(infer(bad_view)
        .unwrap_err()
        .contains("app component must return ui.node"));

    let raw_resource = format!(
        r#"
(def init 0)
(defn update (state action)
  (ui.result state (list (obj "id" "raw")) (list)))
{VIEW}
(ui.app init update app)
"#
    );
    assert!(infer(&raw_resource).unwrap_err().contains("cannot unify"));
}

; This fixture is a differential sequence, not a browser test. Every tree
; assertion compares the regular evaluator's component result with JUIR after
; an update, across Lisp, JSON, YAML, and WS generated forms.
(type Action
  (Toggle int)
  (ShowDone bool))

(def init
  (obj
    "show-done" false
    "todos" (list
      (obj "id" 1 "title" "Plan runtime" "done" false)
      (obj "id" 2 "title" "Ship portable tests" "done" false))))

(def after-toggle
  (obj
    "show-done" false
    "todos" (list
      (obj "id" 1 "title" "Plan runtime" "done" false)
      (obj "id" 2 "title" "Ship portable tests" "done" true))))

(def after-show-done
  (obj
    "show-done" true
    "todos" (list
      (obj "id" 1 "title" "Plan runtime" "done" false)
      (obj "id" 2 "title" "Ship portable tests" "done" true))))

(defn toggle-todo (todo id)
  (if (= (. todo "id") id)
    (obj.set todo "done" (not (. todo "done")))
    todo))

(defn update (state action)
  (case action
    ((Toggle id)
      (obj.set state "todos"
        (list.map (fn (todo) (toggle-todo todo id)) (. state "todos"))))
    ((ShowDone value) (obj.set state "show-done" value))))

(component todo-row (todo)
  (li
    (key (. todo "id"))
    (class "todo-row" "rounded")
    (class-if "is-done" (. todo "done"))
    (input
      (attr type "checkbox")
      (prop checked (. todo "done")))
    (span
      (class-if "line-through" (. todo "done"))
      (text (. todo "title")))))

(component app (state)
  (main
    (class "tasks" "p-4")
    (h1 (text "Tasks"))
    (ul
      (for todo (. state "todos")
        (todo-row todo)))
    (if (. state "show-done")
      (p (class "summary" "visible") (text "Completed tasks shown"))
      (p (class "summary") (text "Completed tasks hidden")))))

(ui.app init update app)

(ui.test "keyed rows, dynamic props, classes, and conditional blocks stay aligned"
  (assert (= init (ui.test.state)))
  (assert (= (app init) (ui.test.tree)))
  (dispatch (Toggle 2))
  (assert (= after-toggle (ui.test.state)))
  (assert (= (app after-toggle) (ui.test.tree)))
  (dispatch (ShowDone true))
  (assert (= after-show-done (ui.test.state)))
  (assert (= (app after-show-done) (ui.test.tree))))

; Fixture-only ui.test scenarios run in the playground's Wasm interpreter.
; They are stripped before the preview is compiled, so they never become app code.
(type Action
  (Increment)
  (Reset))

(def init 0)

(defn update (state action)
  (case action
    ((Increment) (+ state 1))
    ((Reset) 0)))

(component app (state)
  (main
    (class "mx-auto" "max-w-lg" "p-8" "font-sans")
    (div
      (class "rounded-3xl" "bg-slate-900" "p-8" "text-center" "text-white" "shadow-2xl")
      (p (class "text-sm" "font-semibold" "uppercase" "tracking-[0.25em]" "text-cyan-300") (text "Portable UI tests"))
      (p (class "mt-4" "text-7xl" "font-black") (text (str.from state)))
      (div
        (class "mt-6" "flex" "justify-center" "gap-3")
        (button
          (attr type "button")
          (class "rounded-xl" "bg-cyan-400" "px-4" "py-2" "font-bold" "text-slate-950")
          (on click (emit Increment))
          (text "Increment"))
        (button
          (attr type "button")
          (class "rounded-xl" "border" "border-slate-600" "px-4" "py-2" "font-bold")
          (on click (emit Reset))
          (text "Reset"))))))

(ui.app init update app)

(ui.test "counter starts at zero and increments"
  (assert (= 0 (ui.test.state)))
  (assert (= "<main class=\"mx-auto max-w-lg p-8 font-sans\"><div class=\"rounded-3xl bg-slate-900 p-8 text-center text-white shadow-2xl\"><p class=\"text-sm font-semibold uppercase tracking-[0.25em] text-cyan-300\">Portable UI tests</p><p class=\"mt-4 text-7xl font-black\">0</p><div class=\"mt-6 flex justify-center gap-3\"><button class=\"rounded-xl bg-cyan-400 px-4 py-2 font-bold text-slate-950\" type=\"button\">Increment</button><button class=\"rounded-xl border border-slate-600 px-4 py-2 font-bold\" type=\"button\">Reset</button></div></div></main>" (ui.test.html)))
  (dispatch Increment)
  (assert (= 1 (ui.test.state)))
  (dispatch Reset)
  (assert (= 0 (ui.test.state))))

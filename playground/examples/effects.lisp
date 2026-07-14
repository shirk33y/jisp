; This example uses the playground's deliberately small browser effect host.
; storage.write@1 stores JSON in localStorage; timer.tick@1 delivers an int.
(type Action
  (Save)
  (Saved obj)
  (StartClock)
  (StopClock)
  (Tick int)
  (EffectFailed obj))

(def init
  (obj
    "status" "Ready"
    "ticks" 0))

(defn clock (state)
  (ui.subscription "clock" "timer.tick" 1
    (obj "every-ms" 750)
    false
    (ui.action-result "Tick" (list))
    (ui.action-error "EffectFailed" (list))))

(defn update (state action)
  (case action
    ((Save)
      (ui.result
        (obj.set state "status" "Saving to local storage…")
        (list
          (ui.command "save:demo" "storage.write" 1
            (obj "key" "jisp-playground-effect-demo" "value" state)
            true
            (ui.action-result "Saved" (list))
            (ui.action-error "EffectFailed" (list))))
        (list)))
    ((Saved _)
      (ui.result (obj.set state "status" "Saved in local storage") (list) (list)))
    ((StartClock)
      (ui.result (obj.set state "status" "Clock running") (list) (list (clock state))))
    ((StopClock)
      (ui.result (obj.set state "status" "Clock stopped") (list) (list)))
    ((Tick tick)
      (ui.result (obj.set state "ticks" tick) (list) (list (clock state))))
    ((EffectFailed error)
      (ui.result
        (obj.set state "status" (str "Effect failed: " ,(. error "code")))
        (list)
        (list)))))

(component app (state)
  (main
    (class "mx-auto" "max-w-xl" "space-y-5" "rounded-3xl" "bg-slate-950" "p-8" "font-sans" "text-white" "shadow-2xl")
    (div
      (class "space-y-1")
      (p (class "text-xs" "font-semibold" "uppercase" "tracking-[0.25em]" "text-cyan-300") (text "Portable effects"))
      (h1 (class "text-3xl" "font-black") (text "Browser capability host"))
      (p (class "text-sm" "text-slate-300") (text (. state "status"))))
    (div
      (class "rounded-2xl" "bg-slate-900" "p-5")
      (p (class "text-sm" "text-slate-400") (text "Timer deliveries"))
      (p (class "mt-1" "text-5xl" "font-black" "text-cyan-300") (text (str.from (. state "ticks")))))
    (div
      (class "flex" "flex-wrap" "gap-3")
      (button
        (attr type "button")
        (class "rounded-xl" "bg-cyan-400" "px-4" "py-2" "font-bold" "text-slate-950")
        (on click (emit Save))
        (text "Persist state"))
      (button
        (attr type "button")
        (class "rounded-xl" "border" "border-emerald-400" "px-4" "py-2" "font-bold" "text-emerald-200")
        (on click (emit StartClock))
        (text "Start timer"))
      (button
        (attr type "button")
        (class "rounded-xl" "border" "border-slate-600" "px-4" "py-2" "font-bold" "text-slate-200")
        (on click (emit StopClock))
        (text "Stop timer")))))

(ui.app init update app)

; Runs in the playground's portable test runner. `deliver` is the deterministic
; fake-host equivalent of the browser completion above: it never touches storage.
(ui.test "storage completion updates the model"
  (supports "storage.write" 1)
  (dispatch Save)
  (deliver command "save:demo" (obj "key" "jisp-playground-effect-demo"))
  (assert (=
    (obj "status" "Saved in local storage" "ticks" 0)
    (ui.test.state))))

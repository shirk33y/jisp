; `ui.local` owns interaction state per mounted component instance.
; The app reducer stays pure and is not invoked by these disclosure toggles.

(def init null)

(defn update (state action)
  state)

(component disclosure (title body accent)
  (ui.local false (fn (open set-open)
    (section
      (class "rounded-xl" "border" "p-5" "transition-colors")
      (class-if "border-cyan-400" open)
      (class-if "border-slate-700" (not open))
      (button
        (class "flex" "w-full" "items-center" "justify-between" "gap-4" "text-left")
        (on click (emit (set-open (not open))))
        (div
          (p (class "font-semibold" accent) (text title))
          (p (class "mt-1" "text-sm" "text-slate-400")
            (text "Owned by this component instance")))
        (span (class "text-xl" accent) (text (if open "−" "+"))))
      (if open
        (p (class "mt-4" "leading-7" "text-slate-300") (text body))
        (span (text "")))))))

(component app (state)
  (main
    (class "mx-auto" "max-w-3xl" "space-y-4" "p-8" "text-slate-100")
    (div
      (class "mb-8" "space-y-2")
      (p (class "text-sm" "font-medium" "text-cyan-300") (text "COMPONENT-LOCAL STATE"))
      (h1 (class "text-3xl" "font-bold") (text "Three independent disclosures"))
      (p (class "text-slate-400")
        (text "Open any panel. Each value belongs to its own mounted component, not the app model.")))
    (disclosure
      "Why not put this in the reducer?"
      "Ephemeral disclosure state is useful only while this component exists. Keeping it local avoids polluting the serializable app model with presentation details."
      "text-cyan-300")
    (disclosure
      "What happens after unmount?"
      "The executor discards this instance's local cell. If the component mounts again, it starts from its declared initial value."
      "text-violet-300")
    (disclosure
      "What is intentionally deferred?"
      "A keyed dynamic list retains each row's local cell when rows move. An unkeyed list resets its cells whenever the collection changes, so state never silently moves to another row."
      "text-emerald-300")))

(ui.app init update app)

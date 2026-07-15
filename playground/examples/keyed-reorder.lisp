; Focus an input, then press any key to reverse its keyed sibling rows.
; The focused input and every keyed row must retain DOM identity.

(def plan (obj "id" 1 "title" "Plan the release"))
(def build (obj "id" 2 "title" "Build the runtime"))
(def ship (obj "id" 3 "title" "Ship the playground"))
(def init false)

(defn tasks (reversed)
  (if reversed
    (list ship build plan)
    (list plan build ship)))

(defn update (reversed action)
  action)

(component task-row (task reversed)
  (li
    (key (. task "id"))
    (class "rounded-lg" "border" "border-slate-200" "bg-white" "p-3")
    (input
      (attr placeholder (. task "title"))
      (attr "aria-label" (. task "title"))
      (prop value (. task "title"))
      (class "w-full" "rounded" "border" "border-slate-300" "px-2" "py-1")
      (on keydown (emit (not reversed))))))

(component app (reversed)
  (main
    (class "mx-auto" "max-w-xl" "space-y-4" "p-6" "font-sans")
    (div
      (h1 (class "text-2xl" "font-bold") (text "Keyed reorder"))
      (p (class "text-sm" "text-slate-600")
        (text "Focus a task, then press any key. The rows reverse without recreating the focused input.")))
    (button
      (attr type "button")
      (class "rounded-lg" "bg-indigo-600" "px-3" "py-2" "text-sm" "font-semibold" "text-white")
      (on click (emit (not reversed)))
      (text "Reverse tasks"))
    (ul
      (class "space-y-2")
      (for task (tasks reversed)
        (task-row task reversed)))))

(ui.app init update app)

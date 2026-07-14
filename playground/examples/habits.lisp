; A compact daily ritual tracker with a derived completion score.
(type Action
  (Toggle int)
  (Select str))

(def init
  (obj
    "filter" "today"
    "habits" (list
      (obj "id" 1 "name" "Morning pages" "icon" "✎" "streak" 12 "done" true)
      (obj "id" 2 "name" "Strength session" "icon" "↗" "streak" 8 "done" false)
      (obj "id" 3 "name" "Read for 20 minutes" "icon" "◫" "streak" 21 "done" false)
      (obj "id" 4 "name" "Evening walk" "icon" "☼" "streak" 5 "done" true))))

(defn update-habit (habit id)
  (if (= (. habit "id") id)
    (obj.set habit "done" (not (. habit "done")))
    habit))

(defn update (state action)
  (case action
    ((Toggle id) (obj.set state "habits" (list.map (fn (habit) (update-habit habit id)) (. state "habits"))))
    ((Select value) (obj.set state "filter" value))))

(defn visible-habits (state)
  (list.filter
    (fn (habit)
      (or (= (. state "filter") "all") (not (. habit "done"))))
    (. state "habits")))

(defn completed (habits)
  (list.len (list.filter (fn (habit) (. habit "done")) habits)))

(component filter-button (current value label)
  (button
    (attr type "button")
    (class "rounded-full" "px-3" "py-1.5" "text-xs" "font-bold")
    (class-if "bg-violet-600" (= current value))
    (class-if "text-white" (= current value))
    (class-if "bg-violet-100" (not (= current value)))
    (class-if "text-violet-700" (not (= current value)))
    (on click (emit (Select value)))
    (text label)))

(component habit-row (habit)
  (button
    (key (. habit "id"))
    (attr type "button")
    (class "flex" "w-full" "items-center" "gap-3" "rounded-2xl" "border" "border-violet-100" "bg-white" "p-4" "text-left" "shadow-sm" "transition" "hover:-translate-y-0.5")
    (on click (emit (Toggle (. habit "id"))))
    (span
      (class "grid" "size-10" "place-items-center" "rounded-xl" "bg-violet-100" "text-lg" "text-violet-700")
      (text (. habit "icon")))
    (div
      (class "min-w-0" "flex-1")
      (p (class "truncate" "text-sm" "font-bold" "text-slate-900") (text (. habit "name")))
      (p (class "mt-1" "text-xs" "font-medium" "text-slate-500") (text (str "" ,(str.from (. habit "streak")) " day streak"))))
    (span
      (class "grid" "size-7" "place-items-center" "rounded-full" "border-2" "text-sm" "font-black")
      (class-if "border-violet-600" (. habit "done"))
      (class-if "bg-violet-600" (. habit "done"))
      (class-if "text-white" (. habit "done"))
      (class-if "border-violet-200" (not (. habit "done")))
      (text (if (. habit "done") "✓" "")))))

(component app (state)
  (main
    (class "mx-auto" "max-w-xl" "p-6" "font-sans")
    (section
      (class "overflow-hidden" "rounded-3xl" "border" "border-violet-100" "bg-violet-50" "shadow-xl")
      (div
        (class "bg-gradient-to-br" "from-violet-700" "to-fuchsia-600" "p-6" "text-white")
        (p (class "text-xs" "font-bold" "uppercase" "tracking-[0.2em]" "text-violet-200") (text "Daily rhythm"))
        (div
          (class "mt-3" "flex" "items-end" "justify-between")
          (div
            (h1 (class "text-3xl" "font-black") (text "Tiny rituals"))
            (p (class "mt-1" "text-sm" "text-violet-100") (text "Tap a ritual when it is done.")))
          (div
            (class "rounded-2xl" "bg-white/15" "px-4" "py-3" "text-right")
            (p (class "text-2xl" "font-black") (text (str.from (completed (. state "habits")))))
            (p (class "text-xs" "font-bold" "text-violet-100") (text "complete")))))
      (div
        (class "space-y-4" "p-5")
        (div
          (class "flex" "gap-2")
          (filter-button (. state "filter") "today" "Today")
          (filter-button (. state "filter") "all" "All rituals"))
        (div
          (class "space-y-3")
          (for habit (visible-habits state)
            (habit-row habit)))))))

(ui.app init update app)

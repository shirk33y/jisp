; The playground host owns scheduling. Jisp owns immutable state transitions.
(type Action
  (Draft str)
  (Add)
  (Toggle int)
  (Filter str))

(def init
  (obj
    "draft" ""
    "filter" "all"
    "next-id" 4
    "todos" (list
      (obj "id" 1 "title" "Design the update contract" "done" true)
      (obj "id" 2 "title" "Compile Jisp to WebAssembly" "done" false)
      (obj "id" 3 "title" "Ship an interactive playground" "done" false))))

(defn toggle-todo (todo id)
  (if (= (. todo "id") id)
    (obj.set todo "done" (not (. todo "done")))
    todo))

(defn add-todo (state)
  (let (title (. state "draft"))
    (if (= title "")
      state
      (let (todo (obj "id" (. state "next-id") "title" title "done" false))
        (obj.set
          (obj.set
            (obj.set state "todos" (list.append (. state "todos") todo))
            "draft" "")
          "next-id" (+ (. state "next-id") 1))))))

(defn update (state action)
  (case action
    ((Draft value) (obj.set state "draft" value))
    ((Add) (add-todo state))
    ((Toggle id)
      (obj.set state "todos" (list.map (fn (todo) (toggle-todo todo id)) (. state "todos"))))
    ((Filter value) (obj.set state "filter" value))))

(defn visible-todos (state)
  (let (filter (. state "filter"))
    (list.filter
      (fn (todo)
        (or
          (= filter "all")
          (and (= filter "open") (not (. todo "done")))
          (and (= filter "done") (. todo "done"))))
      (. state "todos"))))

(component filter-button (current value label)
  (button
    (attr type "button")
    (class "rounded-lg" "px-3" "py-1.5" "text-sm" "font-medium")
    (class-if "bg-slate-900" (= current value))
    (class-if "text-white" (= current value))
    (class-if "text-slate-600" (not (= current value)))
    (on click (emit (Filter value)))
    (text label)))

(component todo-row (todo)
  (li
    (key (. todo "id"))
    (class "flex" "items-center" "gap-3" "rounded-xl" "border" "border-slate-200" "bg-white" "p-3" "shadow-sm")
    (class-if "opacity-60" (. todo "done"))
    (button
      (attr type "button")
      (attr "aria-label" "Toggle task")
      (class "grid" "size-6" "place-items-center" "rounded-full" "border-2")
      (class-if "border-emerald-500" (. todo "done"))
      (class-if "bg-emerald-500" (. todo "done"))
      (class-if "border-slate-300" (not (. todo "done")))
      (on click (emit (Toggle (. todo "id"))))
      (text (if (. todo "done") "✓" "")))
    (span
      (class "flex-1" "text-sm" "font-medium" "text-slate-800")
      (class-if "line-through" (. todo "done"))
      (text (. todo "title")))))

(component todo-list (todos)
  (ul
    (class "space-y-2")
    (for todo todos
      (todo-row todo))))

(component app (state)
  (main
    (class "mx-auto" "max-w-2xl" "p-6" "font-sans")
    (div
      (class "overflow-hidden" "rounded-2xl" "border" "border-slate-200" "bg-slate-50" "shadow-xl")
      (div
        (class "bg-gradient-to-r" "from-cyan-600" "to-indigo-600" "p-6" "text-white")
          (p (class "text-sm" "font-semibold" "uppercase" "tracking-widest" "text-cyan-100") (text "Update-driven UI"))
        (h1 (class "mt-2" "text-3xl" "font-bold") (text "Jisp tasks"))
        (p (class "mt-2" "text-sm" "text-cyan-50") (text "Events become values; the update function creates the next immutable state.")))
      (div
        (class "space-y-5" "p-5")
        (div
          (class "flex" "gap-2")
          (input
            (attr placeholder "What needs doing?")
            (prop value (. state "draft"))
            (class "min-w-0" "flex-1" "rounded-lg" "border" "border-slate-300" "bg-white" "px-3" "py-2" "text-slate-900" "outline-none" "focus:border-cyan-500")
            (on input (emit (Draft (. event "value")))))
          (button
            (attr type "button")
            (class "rounded-lg" "bg-cyan-600" "px-4" "py-2" "text-sm" "font-semibold" "text-white" "hover:bg-cyan-700")
            (on click (emit Add))
            (text "Add task")))
        (div
          (class "flex" "gap-1" "rounded-xl" "bg-slate-200" "p-1")
          (filter-button (. state "filter") "all" "All")
          (filter-button (. state "filter") "open" "Open")
          (filter-button (. state "filter") "done" "Done"))
        (todo-list (visible-todos state))))))

(ui.app init update app)

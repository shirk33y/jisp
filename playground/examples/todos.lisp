(component todo-row (title)
  (li
    (attr "data-task" title)
    (class "rounded-xl" "border" "border-slate-200" "bg-white" "p-4" "shadow-sm")
    (text title)))

(component app ()
  (ul
    (class "mx-auto" "max-w-xl" "space-y-3")
    (for title (list "Review UI syntax" "Ship the playground" "Design state runtime")
      (todo-row title))))

(export main
  (fn ()
    (ui.html (app))))

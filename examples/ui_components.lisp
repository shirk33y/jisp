; Declarative UI components are renderer-neutral structural nodes.
(component todo-row (title)
  (li
    (attr "data-id" title)
    (class "rounded" "px-2")
    (span (text title))))

(component todo-list (titles)
  (ul
    (attr "aria-label" "Tasks")
    (for title titles
      (todo-row title))))

(export main
  (fn ()
    (ui.html (todo-list (list "Plan" "Ship")))))

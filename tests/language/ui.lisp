(def saving false)

(component todo-row (title)
  (li
    (attr "data-id" title)
    (class "rounded" "px-2")
    (class-if "opacity-50" saving)
    (span (text title))))

(component todo-list (titles)
  (ul
    (attr "aria-label" "Tasks")
    (for title titles
      (todo-row title))))

(test "ui components render explicit attributes classes and repeated children"
  (assert
    (= "<ul aria-label=\"Tasks\"><li class=\"rounded px-2\" data-id=\"Plan\"><span>Plan</span></li><li class=\"rounded px-2\" data-id=\"Ship\"><span>Ship</span></li></ul>"
      (ui.html (todo-list (list "Plan" "Ship"))))))

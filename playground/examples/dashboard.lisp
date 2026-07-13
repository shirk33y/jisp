(component metric (value)
  (div
    (class "rounded-2xl" "bg-white" "p-5" "text-2xl" "font-bold" "text-slate-900" "shadow-sm")
    (text value)))

(component app ()
  (section
    (class "mx-auto" "grid" "max-w-4xl" "gap-4" "md:grid-cols-3")
    (for value (list "2,481 users" "3m 12s builds" "94.6% coverage")
      (metric value))))

(export main
  (fn ()
    (ui.html (app))))

(component field (value)
  (input
    (attr "aria-label" value)
    (attr "value" value)
    (class "block" "w-full" "rounded-xl" "border" "border-slate-300" "bg-white" "px-3" "py-2" "text-slate-900")))

(component app ()
  (form
    (class "mx-auto" "max-w-xl" "space-y-3" "rounded-3xl" "bg-white" "p-7" "shadow-xl")
    (for value (list "Project name: jisp" "Preview host: GitHub Pages" "Theme: system")
      (field value))))

(export main
  (fn ()
    (ui.html (app))))

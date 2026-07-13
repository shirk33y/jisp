(component profile-line (value)
  (div
    (class "rounded-xl" "bg-slate-100" "p-4" "font-semibold" "text-slate-800")
    (text value)))

(component app ()
  (section
    (class "mx-auto" "max-w-lg" "space-y-3" "rounded-3xl" "bg-white" "p-7" "shadow-xl")
    (for value (list "Jisp Studio" "Portable UI experiments" "12 components")
      (profile-line value))))

(export main
  (fn ()
    (ui.html (app))))

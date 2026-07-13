(component project (name)
  (article
    (class "rounded-2xl" "border" "border-slate-200" "bg-white" "p-5" "font-semibold" "text-slate-900" "shadow-sm")
    (key name)
    (text name)))

(component app ()
  (section
    (class "mx-auto" "grid" "max-w-4xl" "gap-4" "md:grid-cols-3")
    (for name (list "Compiler" "Playground" "Documentation")
      (project name))))

(export main
  (fn ()
    (ui.html (app))))

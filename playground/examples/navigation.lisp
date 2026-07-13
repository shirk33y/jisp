(component nav-item (label)
  (a
    (attr "href" "#")
    (class "rounded-lg" "bg-white" "px-3" "py-2" "text-sm" "font-semibold" "text-slate-900")
    (text label)))

(component app ()
  (nav
    (class "mx-auto" "flex" "max-w-3xl" "gap-2" "rounded-2xl" "bg-slate-900" "p-3")
    (for label (list "Overview" "Components" "Settings")
      (nav-item label))))

(export main
  (fn ()
    (ui.html (app))))

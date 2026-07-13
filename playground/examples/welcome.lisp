(component app ()
  (div
    (class "rounded-3xl" "bg-white" "p-8" "shadow-xl")
    (text "Jisp UI — one tree, many hosts.")))

(export main
  (fn ()
    (ui.html (app))))

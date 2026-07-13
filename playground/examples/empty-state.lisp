(component app ()
  (div
    (class "mx-auto" "max-w-md" "rounded-3xl" "bg-cyan-100" "p-10" "text-center" "text-xl" "font-bold" "text-cyan-900")
    (text "Nothing here yet. Start with a component.")))

(export main
  (fn ()
    (ui.html (app))))

(component app ()
  (article
    (class "mx-auto" "max-w-sm" "rounded-3xl" "bg-gradient-to-br" "from-violet-500" "to-fuchsia-500" "p-8" "text-2xl" "font-bold" "text-white" "shadow-xl")
    (text "Orbit desk lamp — $49")))

(export main
  (fn ()
    (ui.html (app))))

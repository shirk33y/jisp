(component notice (message active)
  (article
    (class "rounded-2xl" "border" "p-4" "shadow-sm")
    (class-if "border-emerald-300" active)
    (class-if "bg-emerald-50" active)
    (text message)))

(component app ()
  (section
    (class "mx-auto" "max-w-lg" "space-y-3")
    (notice "Deployment complete" true)
    (notice "The HTML renderer escapes text and attributes." false)
    (notice "Interactive events are a future host runtime feature." false)))

(export main
  (fn ()
    (ui.html (app))))

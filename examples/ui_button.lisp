(def user-active true)
(def saving false)
(def blog-title "Draft editor")

(def button
  (obj
    "tag" "button"
    "id" "save-button"
    "title" blog-title
    "classes"
      (obj
        "px-4" true
        "py-2" true
        "opacity-50" saving
        "bg-emerald-600" (and user-active (not saving)))
    "children"
      (list
        (obj
          "tag" "text"
          "value" "Save"))))

(export main
  (fn ()
    button))

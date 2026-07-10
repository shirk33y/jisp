(def user-active true)
(def saving false)
(def blog-title (str "Draft editor"))

(def button
  (obj
    (str "tag") (str "button")
    (str "id") (str "save-button")
    (str "title") blog-title
    (str "classes")
      (obj
        (str "px-4") true
        (str "py-2") true
        (str "opacity-50") saving
        (str "bg-emerald-600") (and user-active (not saving)))
    (str "children")
      (list
        (obj
          (str "tag") (str "text")
          (str "value") (str "Save")))))

(export main button)

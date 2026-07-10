(def profile
  (obj
    (str "name") (str "Ada")
    (str "age") 37))

(def renamed (obj.set profile (str "name") (str "Grace")))
(def public-profile (obj.del renamed (str "age")))
(def flags (obj (str "active") true))
(def combined (obj.cat public-profile flags))
(def range (obj (str "start") 1 (str "end") 3))

(export main
  (fn ()
    (if (and (obj.has combined (str "active"))
             (= (obj.len combined) (list.len (obj.keys combined))))
      (+ (list.len (obj.values range)) (obj.len public-profile))
      0)))

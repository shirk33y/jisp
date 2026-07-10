(def profile
  (obj
    "name" "Ada"
    "age" 37))

(def renamed (obj.set profile "name" "Grace"))
(def public-profile (obj.del renamed "age"))
(def flags (obj "active" true))
(def combined (obj.cat public-profile flags))
(def range (obj "start" 1 "end" 3))

(export main
  (fn ()
    (if (and (obj.has combined "active")
             (= (obj.len combined) (list.len (obj.keys combined))))
      (+ (list.len (obj.values range)) (obj.len public-profile))
      0)))

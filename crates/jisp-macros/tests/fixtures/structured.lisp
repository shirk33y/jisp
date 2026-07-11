(def stats (obj "active" true "age" 41))
(def numbers (list.append (list.prepend 1 (list 2 3)) 4))

(export structured-entry
  (fn ()
    (+
      (case stats
        ((obj "active" true "age" age) age)
        (_ 0))
      (list.len numbers))))

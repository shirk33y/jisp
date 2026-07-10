(def answer (fn () (+ 40 2)))
(def stats (obj (str "active") true (str "age") 41))

(export main
  (fn ()
    (+
      (case (list 1 40 99)
        ((list 1 value ... tail) (+ value 1))
        (_ 0))
      (case stats
        ((obj "active" true "age" age) (+ age 1))
        (_ (answer))))))

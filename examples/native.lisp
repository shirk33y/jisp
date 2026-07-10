(def answer (fn () (+ 40 2)))
(def stats (obj (str "active") true (str "age") 41))
(def words (str.split (str "jisp,native,rust") (str ",")))
(def label (str.cat (str.upper (str "p1")) (str ":") (str.join (str "-") words)))
(def numbers (list.append (list.prepend 1 (list 2 3)) 4))

(export main
  (fn ()
    (+
      (case (list 1 40 99)
        ((list 1 value ... tail) (+ value (list.len tail)))
        (_ 0))
      (+
        (case stats
          ((obj "active" true "age" age) (+ age 1))
          (_ (answer)))
        (+ (str.len label) (list.len (list.rest numbers)))))))

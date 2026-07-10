(def answer (fn () (+ 40 2)))
(def stats (obj "active" true "age" 41))
(def words (str.split "jisp,native,rust" ","))
(def label (str.cat (str.upper "p1") ":" (str.join "-" words)))
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

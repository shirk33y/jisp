(import rules "rules")

(type quote-result
  (quoted int)
  (invalid str))

(def price
  (fn (rate ... prices)
    (let (total (rules.subtotal prices))
      (if (> total 0)
        (quoted (rules.apply-discount rate total))
        (invalid "empty quote")))))

(export main
  (fn ()
    (case (price 1 20 30 -8)
      ((quoted total) total)
      ((invalid _) 0))))

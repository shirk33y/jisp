(export subtotal
  (fn (prices)
    (list.fold (fn (total price) (+ total price)) 0 prices)))

(export apply-discount
  (fn (rate total)
    (- total (* total rate))))

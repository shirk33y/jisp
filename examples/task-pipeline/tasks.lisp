(export weighted-total
  (fn (weights effort urgency)
    (let (score (list.fold (fn (total value) (+ total value)) 0 weights))
      (+ score (+ effort urgency)))))

(export normalize-title
  (fn (title)
    (str.upper (str.trim title))))

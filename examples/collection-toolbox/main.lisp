(def add-one
  (fn (value)
    (+ value 1)))

(def sum
  (fn (total value)
    (+ total value)))

(export main
  (fn ()
    (let (original (list 1 2)
          updated (list.append (list.map add-one original) 3)
          scores (map "first" 10 "second" 20)
          with-third (map.set scores "third" 7))
      (+ (list.fold sum 0 updated)
         (case (map.get with-third "third")
           ((ok value) value)
           ((err _) 0))))))

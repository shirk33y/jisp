(def twice
  (~ (fn (value)
       `(+ ,value ,value))))

(export main
  (fn ()
    (let (value 21)
      (twice value))))

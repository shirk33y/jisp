(def score-of
  (fn (record)
    (. record "score")))

(export main
  (fn ()
    (score-of (obj "score" 42))))

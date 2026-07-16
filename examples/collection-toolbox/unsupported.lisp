(def mixed
  (obj "name" "Ada" "score" 42))

(export main
  (fn ()
    (let (key (str.cat "sc" "ore"))
      (. mixed key))))

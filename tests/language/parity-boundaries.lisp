(def nested-data
  (fn ()
    (obj
      "record" (obj "count" 40 "items" (list 1 2))
      "rows" (list (list 3 4) (list 5 6)))))

(def swap-pair
  (~ (fn (left right)
       `(list ,right ,left))))

(test "nested list and object data remain values"
  (assert (=
    42
    (let (value (nested-data))
      (+ (. (. value "record") "count") (list.len (. value "rows")))))))

(test "unicode and escaped strings stay data"
  (assert (=
    "quote:\" newline:\n🙂"
    (str "quote:\" newline:\n" "🙂"))))

(test "quote and unquote normalize before lowering"
  (assert (= (list 41 1) (swap-pair 1 41))))

(test-error "type failures keep a portable category"
  "no overload of `+`"
  (+ 1 true))

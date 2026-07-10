(def parse-count
  (fn (raw)
    (case raw
      ("zero" (ok 0))
      ("one" (ok 1))
      ("two" (ok 2))
      (_ (err (str "bad-count:" ,raw))))))

(def double-count
  (fn (raw)
    (result.map
      (parse-count raw)
      (fn (value)
        (* value 2)))))

(def label-count
  (fn (raw)
    (result.try
      (parse-count raw)
      (fn (value)
        (ok (str "count:" ,(str.from value)))))))

(def normalize-count
  (fn (raw)
    (result.recover
      (result.map-err
        (parse-count raw)
        (fn (message)
          (str.upper message)))
      (fn (message)
        (ok (str.len message))))))

(def render-name
  (fn (maybe)
    (case maybe
      ((some name) name)
      ((none) "anonymous"))))

(test "result map transforms ok and preserves err"
  (assert.equal
    (list (ok 4) (err "bad-count:many"))
    (list
      (double-count "two")
      (double-count "many"))))

(test "result try chains ok and short circuits err"
  (assert.equal
    (list (ok "count:1") (err "bad-count:nope"))
    (list
      (label-count "one")
      (label-count "nope"))))

(test "result map-err and recover normalize failures"
  (assert.equal
    (list (ok 2) (ok 14))
    (list
      (normalize-count "two")
      (normalize-count "many"))))

(test "option constructors participate in exhaustive case"
  (assert.equal
    (list "Ada" "anonymous")
    (list
      (render-name (some "Ada"))
      (render-name none))))

(test "use desugars callback-last result propagation"
  (assert.equal
    (ok 3)
    (use value (result.try (parse-count "two"))
      (ok (+ value 1)))))

(type response
  (success int)
  (failure str))

(def stats
  (obj "active" true "score" 40))

(def words
  (list "ada" "lin" "grace"))

(def scores
  (obj "primary" 40 "secondary" 41))

(def increment
  (fn (value)
    (+ value 1)))

(def decrement
  (fn (value)
    (- value 1)))

(def above-one
  (fn (value)
    (> value 1)))

(def sum
  (fn (total value)
    (+ total value)))

(def apply-twice
  (fn (callback value)
    (+ 0 (callback (callback value)))))

(def sum-rest
  (fn (head ... tail)
    (+ head (list.fold sum 0 tail))))

(def make-rest-adder
  (fn (offset)
    (fn (head ... tail)
      (+ offset (+ head (list.fold sum 0 tail))))))

(def make-adder
  (fn (offset)
    (fn (value)
      (+ value offset))))

(def wrap
  (fn (value)
    (list (+ value 0))))

(def as-list-error
  (fn (message)
    (list (str.cat message ""))))

(def failed-result
  (fn ()
    (if false
      (ok 0)
      (err "bad"))))

(def retry-with-list
  (fn (value)
    (list.slice (list (+ value 0)) 0 1)))

(def recover-list-error
  (fn (message)
    (if true
      (ok 42)
      (err (list (str.cat message ""))))))

(export scalar-entry
  (fn ()
    (+ 20 22)))

(export object-field-entry
  (fn ()
    (+ (. stats "score") 2)))

(export object-get-entry
  (fn ()
    (obj.get stats "score")))

(export object-get-discarded-entry
  (fn ()
    (do
      (obj.get stats "score")
      42)))

(export object-get-case-entry
  (fn ()
    (case (obj.get stats "score")
      ((ok value) (+ value 2))
      ((err _) 0))))

(export inline-object-get-entry
  (fn ()
    (case (obj.get (obj "score" 40) "score")
      ((ok value) (+ value 2))
      ((err _) 0))))

(export option-case-entry
  (fn ()
    (case (some 41)
      ((some value) (+ value 1))
      ((none) 0))))

(export result-map-entry
  (fn ()
    (case (result.map (obj.get stats "score") wrap)
      ((ok values) (list.len values))
      ((err _) 0))))

(export result-map-err-entry
  (fn ()
    (case (result.map-err (failed-result) as-list-error)
      ((ok _) 0)
      ((err messages) (list.len messages)))))

(export result-try-entry
  (fn ()
    (case (result.try (obj.get stats "score") retry-with-list)
      ((ok values) (list.len values))
      ((err _) 0))))

(export result-recover-entry
  (fn ()
    (case (result.recover (failed-result) recover-list-error)
      ((ok value) value)
      ((err _) 0))))

(export boolean-entry
  (fn ()
    (and (. stats "active") (list.has words "lin"))))

(export string-entry
  (fn ()
    (str "users:" ,(str.join "-" words))))

(export list-entry
  (fn ()
    (list.append (list.prepend 1 (list 2 3)) 4)))

(export map-entry
  (fn ()
    (list.map increment (list 1 2 3))))

(export filter-entry
  (fn ()
    (list.filter above-one (list 1 2 3))))

(export fold-entry
  (fn ()
    (list.fold sum 0 (list 1 2 3))))

(export some-entry
  (fn ()
    (+ 40 (if (list.some above-one (list 0 1 2)) 2 0))))

(export every-entry
  (fn ()
    (+ 40 (if (list.every above-one (list 2 3 4)) 2 0))))

(export higher-order-entry
  (fn ()
    (apply-twice increment 40)))

(export first-class-call-entry
  (fn ()
    ((if true increment decrement) 41)))

(export variadic-empty-entry
  (fn ()
    (sum-rest 42)))

(export variadic-many-entry
  (fn ()
    (sum-rest 40 1 1)))

(export variadic-local-entry
  (fn ()
    (let (offset 40
          add-rest (fn (head ... tail)
                     (+ offset (+ head (list.fold sum 0 tail)))))
      (add-rest 1 1))))

(export variadic-expression-entry
  (fn ()
    ((if true sum-rest sum-rest) 40 1 1)))

(export variadic-returned-entry
  (fn ()
    ((make-rest-adder 40) 1 1)))

(export local-function-entry
  (fn ()
    (let (add-one (fn (value) (+ value 1)))
      (add-one 41))))

(export immediate-lambda-entry
  (fn ()
    ((fn (value) (+ value 1)) 41)))

(export captured-map-entry
  (fn ()
    (let (offset 40)
      (list.map (fn (value) (+ value offset)) (list 1 2)))))

(export captured-string-entry
  (fn ()
    (let (prefix "a"
          append-prefix (fn (value) (str.cat prefix value)))
      (str.cat prefix (append-prefix "b")))))

(export captured-string-map-entry
  (fn ()
    (let (prefix "a"
          append-prefix (fn (value) (str.cat prefix value)))
      (list.map append-prefix (list "b" "c")))))

(export captured-use-entry
  (fn ()
    (let (offset 2)
      (case
        (use value (result.try (obj.get stats "score"))
          (ok (+ value offset)))
        ((ok value) value)
        ((err _) 0)))))

(export returned-closure-entry
  (fn ()
    ((make-adder 1) 41)))

(export returned-closure-map-entry
  (fn ()
    (list.map (make-adder 40) (list 1 2))))

(export enum-case-entry
  (fn ()
    (case (success 41)
      ((success value) (+ value 1))
      ((failure _) 0))))

(export bigint-sum-entry
  (fn ()
    (+ (bigint "9223372036854775808") (bigint "2"))))

(export bigint-floor-entry
  (fn ()
    (// (bigint "-5") (bigint "2"))))

(export bigint-modulo-entry
  (fn ()
    (% (bigint "-5") (bigint "2"))))

(export bigint-abs-entry
  (fn ()
    (math.abs (bigint "-9223372036854775808"))))

(export bigint-minmax-entry
  (fn ()
    (+
      (math.min (bigint "4") (bigint "9"))
      (math.max (bigint "4") (bigint "9")))))

(export bigint-comparison-entry
  (fn ()
    (if (> (bigint "9223372036854775808") (bigint "9223372036854775807"))
      42
      0)))

(export bigint-closure-entry
  (fn ()
    (let (offset (bigint "9223372036854775808")
          add-offset (fn (value) (+ value offset)))
      (add-offset (bigint "2")))))

(export dynamic-field-entry
  (fn ()
    (let (key (str.cat "pri" "mary"))
      (+ (. scores key) 2))))

(export dynamic-object-get-entry
  (fn ()
    (let (key (str.cat "sec" "ondary"))
      (case (obj.get scores key)
        ((ok value) (+ value 1))
        ((err _) 0)))))

(export dynamic-object-get-missing-entry
  (fn ()
    (let (key (str.cat "mis" "sing"))
      (case (obj.get scores key)
        ((ok _) 0)
        ((err _) 42)))))

(export dynamic-object-has-entry
  (fn ()
    (let (key (str.cat "sec" "ondary"))
      (if (obj.has scores key) 42 0))))

(export dynamic-object-set-entry
  (fn ()
    (let (key (str.cat "sec" "ondary")
          updated (obj.set scores key 42))
      (. updated "secondary"))))

(export dynamic-object-set-immutable-entry
  (fn ()
    (let (key (str.cat "sec" "ondary")
          original scores
          updated (obj.set original key 42))
      (+ (. original "secondary") (. updated "secondary")))))

(export object-del-immutable-entry
  (fn ()
    (let (original (obj "first" 40 "second" 2)
          updated (obj.del original "second"))
      (+ (. original "second") (. updated "first")))))

(export nested-alternative-list-entry
  (fn ()
    (case (list 2 40)
      ((list (or 1 2) value) (+ value 2))
      (_ 0))))

(export nested-alternative-object-entry
  (fn ()
    (case (obj "kind" "two" "value" 40)
      ((obj "kind" (or "one" "two") "value" value) (+ value 2))
      (_ 0))))

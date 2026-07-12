(type response
  (success int)
  (failure str))

(def stats
  (obj "active" true "score" 40))

(def words
  (list "ada" "lin" "grace"))

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

(export enum-case-entry
  (fn ()
    (case (success 41)
      ((success value) (+ value 1))
      ((failure _) 0))))

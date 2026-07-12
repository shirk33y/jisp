(type response
  (ok int)
  (err str))

(def stats
  (obj "active" true "score" 40))

(def words
  (list "ada" "lin" "grace"))

(def increment
  (fn (value)
    (+ value 1)))

(def above-one
  (fn (value)
    (> value 1)))

(def sum
  (fn (total value)
    (+ total value)))

(def apply-twice
  (fn (callback value)
    (+ 0 (callback (callback value)))))

(export scalar-entry
  (fn ()
    (+ 20 22)))

(export object-field-entry
  (fn ()
    (+ (. stats "score") 2)))

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

(export enum-case-entry
  (fn ()
    (case (ok 41)
      ((ok value) (+ value 1))
      ((err _) 0))))

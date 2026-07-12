(type response
  (ok int)
  (err str))

(def stats
  (obj "active" true "score" 40))

(def words
  (list "ada" "lin" "grace"))

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

(export enum-case-entry
  (fn ()
    (case (ok 41)
      ((ok value) (+ value 1))
      ((err _) 0))))

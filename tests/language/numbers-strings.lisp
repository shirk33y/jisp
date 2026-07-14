(def arithmetic-report
  (fn (left right)
    (list
      (+ left right)
      (- left right)
      (* left right)
      (/ left right)
      (// left right)
      (% left right))))

(def normalize-name
  (fn (raw)
    (str.replace
      (str.lower (str.trim raw))
      " "
      "-")))

(def csv-row
  (fn (cells)
    (str.join "," (list.map str.from cells))))

(test "integer division modes are explicit in portable lisp"
  (assert (=
    (list -4 -10 -21 -2 -3 2)
    (arithmetic-report -7 3))))

(test "float math helpers compose without integer coercion"
  (assert (=
    (list 3.5 9.0 4.0 4.0 4.0)
    (list
      (math.abs -3.5)
      (math.sqrt 81.0)
      (math.floor 4.8)
      (math.ceil 3.2)
      (math.round 3.5)))))

(test "string predicates and replacement build slugs"
  (assert (=
    (obj
      "slug" "ada-lovelace"
      "has" true
      "starts" true
      "ends" true
      "is-number-string" false)
    (let (slug (normalize-name "  Ada Lovelace  "))
      (obj
        "slug" slug
        "has" (str.has slug "love")
        "starts" (str.starts slug "ada")
        "ends" (str.ends slug "lace")
        "is-number-string" (str.is 42))))))

(test "string split join and conversion cooperate"
  (assert (=
    "1,2,3"
    (csv-row (list 1 2 3)))))

(test "string templates interpolate and splice"
  (assert (=
    "hello ADA and Lin"
    (str "hello " ,(str.upper "ada") " and " ,@(list "Lin")))))

(test "string lines splice list items with newlines"
  (assert (=
    "one\ntwo\nthree"
    (str.lines "one" ,@(str.split "two,three" ",")))))

(test "string slice returns ok or err values"
  (assert (=
    (list (ok "bcd") (err "string slice is out of bounds"))
    (list
      (str.slice "abcdef" 1 4)
      (str.slice "abc" 0 9)))))

(def base-user
  (fn ()
    (obj
      "id" 7
      "name" "Ada"
      "active" true
      "score" 41)))

(def rename-user
  (fn (user name)
    (obj.set user "name" name)))

(def public-user
  (fn (user)
    (obj.del user "score")))

(def button-view
  (fn (user)
    (let (active (. user "active"))
      (obj
        "tag" "button"
        "class" (obj
          "px-2" true
          "opacity-50" (not active)
          "text-green" active)
        "attrs" (obj
          "id" (str "user-" ,(str.from (. user "id")))
          "title" (. user "name"))
        "children" (list
          (obj "tag" "text" "value" (. user "name")))))))

(def summarize-user
  (fn (user)
    (case user
      ((obj "active" true "name" name "score" score)
        (str ,name ":" ,(str.from score) ":active"))
      ((obj "active" false "name" name)
        (str ,name ":inactive")))))

(test "field access reads statically known object keys"
  (assert.equal
    (obj "id" 7 "name" "Ada" "active" true)
    (let (user (base-user))
      (obj "id" (. user "id") "name" (. user "name") "active" (. user "active")))))

(test "object set and delete are immutable data transforms"
  (assert.equal
    (obj "original" "Ada" "renamed" "Grace" "has-score" false)
    (let (original (base-user)
          renamed (rename-user original "Grace")
          public (public-user renamed))
      (obj
        "original" (. original "name")
        "renamed" (. renamed "name")
        "has-score" (obj.has public "score")))))

(test "object get returns ok and missing key err values"
  (assert.equal
    (list (ok 41) (err "object has no key `missing`"))
    (let (user (base-user))
      (list
        (obj.get user "score")
        (obj.get user "missing")))))

(test "object cat overlays later keys"
  (assert.equal
    (obj "name" "Ada" "role" "admin" "active" false)
    (obj.cat
      (obj "name" "Ada" "role" "guest")
      (obj "role" "admin" "active" false))))

(test "object keys and values preserve insertion order"
  (assert.equal
    (obj "keys" (list "first" "second" "third") "values" (list 1 2 3))
    (let (value (obj "first" 1 "second" 2 "third" 3))
      (obj "keys" (obj.keys value) "values" (obj.values value)))))

(test "homogeneous objects convert explicitly to dynamic maps"
  (assert.equal
    (obj "has-primary" false "secondary" (ok 2))
    (let (scores (obj.to-map (obj "primary" 1 "secondary" 2))
          without-primary (map.del scores "primary"))
      (obj
        "has-primary" (map.has without-primary "primary")
        "secondary" (map.get without-primary "secondary")))))

(test "object patterns refine boolean fields"
  (assert.equal
    (list "Ada:41:active" "Lin:inactive")
    (list
      (summarize-user (base-user))
      (summarize-user (obj "name" "Lin" "active" false "score" 9)))))

(test "ui shaped objects keep utility classes as boolean maps"
  (assert.equal
    (obj
      "tag" "button"
      "class" (obj
        "px-2" true
        "opacity-50" false
        "text-green" true)
      "attrs" (obj
        "id" "user-7"
        "title" "Ada")
      "children" (list
        (obj "tag" "text" "value" "Ada")))
    (button-view (base-user))))

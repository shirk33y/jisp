type route
  (home)
  profile int str
  search str
  missing str

def render-route
  fn (route)
    case route
      (home)
        "/"
      (profile id tab)
        str "/users/" ,(str.from id) "/" ,tab
      (search query)
        str "/search?q=" ,query
      (missing path)
        str "404:" ,path

def nested-list-score
  fn (value)
    case value
      (list (list 1 points) ... rest)
        + points
          list.len rest
      (list)
        0
      _ -1

def nested-object-state
  fn (event)
    case event
      (obj "kind" "click" "meta" (obj "trusted" true "count" count))
        str "trusted:" ,(str.from count)
      (obj "kind" "click" "meta" (obj "trusted" false))
        "blocked"
      (obj "kind" "load" "meta" _)
        "loaded"
      _ "ignored"

def literal-match
  fn (value)
    case value
      0 "zero"
      1 "one"
      _ "many"

def guarded-grade
  fn (score)
    case score
      (when value (>= value 90))
        "high"
      (when value (>= value 60))
        "pass"
      _ "retry"

test "variant case binds payload fields"
  assert.equal
    list "/" "/users/42/posts" "/search?q=jisp" "404:/old"
    list
      render-route home
      render-route
        profile 42 "posts"
      render-route
        search "jisp"
      render-route
        missing "/old"

test "list patterns can nest and bind rest"
  assert.equal
    list 5 0 -1
    list
      nested-list-score
        list
          list 1 3
          list 9 9
          list 10 10
      nested-list-score
        (list)
      nested-list-score
        list
          list 2 3

test "object patterns can nest and refine literals"
  assert.equal
    list "trusted:3" "blocked" "loaded" "ignored"
    list
      nested-object-state
        obj "kind" "click"
          ... "meta" (obj "trusted" true "count" 3)
      nested-object-state
        obj "kind" "click"
          ... "meta" (obj "trusted" false "count" 2)
      nested-object-state
        obj "kind" "load"
          ... "meta" (obj "trusted" true "count" 0)
      nested-object-state
        obj "kind" "hover"
          ... "meta" (obj "trusted" true "count" 0)

test "literal and wildcard branches keep case exhaustive"
  assert.equal
    list "zero" "one" "many"
    list
      literal-match 0
      literal-match 1
      literal-match 9

test "case guards dispatch before unguarded fallback"
  assert.equal
    list "high" "pass" "retry"
    list
      guarded-grade 99
      guarded-grade 75
      guarded-grade 40

test "or patterns share payload bindings across alternatives"
  assert.equal 8
    case
      search "ignored"
      (or (search value) (missing value))
        +
          str.len value
          1
      (home)
        0
      (profile _ _)
        0

test-error "redundant guarded branch after catch-all is rejected"
  "redundant case pattern"
  case true
    _ 1
    (when false true)
      0

test-error "non-exhaustive finite list case is rejected"
  "non-exhaustive case for `list`"
  case
    list true
    (list true)
      1

test-error "raw macro-import must be resolved before lowering"
  "macro-import must be resolved before lowering"
  module
    macro-import macros "macros.lisp"

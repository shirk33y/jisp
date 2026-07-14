def maybe-divide
  fn (enabled left right)
    if enabled
      / left right
      99

def divides-to-two
  fn (left right)
    and
      not
        = right 0
      =
        / left right
        2

def prefer-cached
  fn (cached left right)
    or cached
      =
        / left right
        1

def collect-tail
  fn (head ... tail)
    obj "head" head
      ... "tail" tail
      ... "tail-len" (list.len tail)

def pair-tail
  fn (values)
    case values
      (list first second ... tail)
        obj "first" first
          ... "second" second
          ... "tail" tail
      _
        obj "first" 0
          ... "second" 0
          ... "tail" (list)

test "if and boolean forms do not evaluate dead divide-by-zero branches"
  assert
    =
      obj "if-false" 99
        ... "and-false" false
        ... "or-true" true
      obj "if-false" (maybe-divide false 1 0)
        ... "and-false" (divides-to-two 4 0)
        ... "or-true" (prefer-cached true 1 0)

test "result callbacks are not evaluated on preserved branches"
  assert
    =
      obj "map" (err "nope")
        ... "try" (err "nope")
        ... "recover" (ok 12)
      obj "map"
        result.map
          err "nope"
          fn (value)
            / value 0
        "try"
        result.try
          err "nope"
          fn (value)
            ok
              / value 0
        "recover"
        result.recover
          ok 12
          fn (message)
            ok
              / 1 0

test "unicode string length and slice use character boundaries"
  assert
    =
      obj "len" 6
        ... "slice" (ok "żó")
        ... "empty" (ok "")
        ... "reverse" (err "string slice is out of bounds")
        ... "negative" (err "string slice indices cannot be negative")
      let (value "zażółć")
        obj "len" (str.len value)
          ... "slice" (str.slice value 2 4)
          ... "empty" (str.slice value 3 3)
          ... "reverse" (str.slice value 4 2)
          ... "negative" (str.slice value -1 2)

test "list bounds distinguish negative reverse and empty-at-end slices"
  assert
    =
      obj "negative-get" (err "list index cannot be negative")
        ... "negative-slice" (err "list slice indices cannot be negative")
        ... "reverse-slice" (err "list slice is out of bounds")
        ... "empty-at-end" (ok (list))
      let (values (list 1 2 3))
        obj "negative-get" (list.get values -1)
          ... "negative-slice" (list.slice values -1 2)
          ... "reverse-slice" (list.slice values 2 1)
          ... "empty-at-end" (list.slice values 3 3)

test "variadic functions and rest patterns bind empty tails"
  assert
    =
      obj "function"
        obj "head" "solo"
          ... "tail" (list)
          ... "tail-len" 0
        "pattern"
        obj "first" 10
          ... "second" 20
          ... "tail" (list)
      obj "function" (collect-tail "solo")
        ... "pattern" (pair-tail (list 10 20))

test "object updates keep order and missing deletes are no-ops"
  assert
    =
      obj "keys" (list "a" "b" "c")
        ... "values" (list 9 22 3)
        ... "missing-delete" (obj "a" 1 "b" 2)
      let
        base
          obj "a" 1
            ... "b" 2
          updated
          obj.set
            obj.cat base
              obj "b" 22
                ... "c" 3
            "a"
            9
        obj "keys" (obj.keys updated)
          ... "values" (obj.values updated)
          ... "missing-delete" (obj.del base "missing")

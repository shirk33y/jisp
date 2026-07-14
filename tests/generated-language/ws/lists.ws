def summarize-list
  fn (values)
    obj "len" (list.len values)
      ... "first" (list.first values)
      ... "last" (list.last values)
      ... "rest" (list.rest values)

def classify-list
  fn (values)
    case values
      (list)
        "empty"
      (list one)
        str "single:" ,(str.from one)
      (list first second ... tail)
        str
          "many:"
          ,(str.from first)
          ":"
          ,(str.from second)
          ":"
          ,(str.from (list.len tail))

def square-evens-total
  fn (values)
    list.fold
      fn (total value)
        + total value
      0
      list.map
        fn (value)
          * value value
        list.filter
          fn (value)
            =
              % value 2
              0
          values

test "list shape helpers return values and result edges"
  assert
    =
      obj "len" 3
        ... "first" (ok 4)
        ... "last" (ok 8)
        ... "rest" (list 6 8)
      summarize-list
        list 4 6 8

test "empty list first and last return err values"
  assert
    =
      obj "len" 0
        ... "first" (err "list is empty")
        ... "last" (err "list is empty")
        ... "rest" (list)
      summarize-list
        (list)

test "list get and slice report bounds as result values"
  assert
    =
      obj "hit" (ok 20)
        ... "miss" (err "list index is out of bounds")
        ... "slice" (ok (list 20 30))
      obj "hit"
        list.get
          list 10 20 30 40
          1
        ... "miss" (list.get (list 10 20) 7)
        "slice"
        list.slice
          list 10 20 30 40
          1
          3

test "list concatenation prepend and append preserve order"
  assert
    =
      list 0 1 2 3 4 5
      list.cat
        list.prepend 0
          list 1 2
        list.append
          list 3 4
          5

test "list updates do not mutate an aliased input"
  assert
    =
      obj "original" (list 1 2)
        ... "updated" (list 1 2 3)
      let
        original
          list 1 2
          updated
          list.append original 3
        obj "original" original
          ... "updated" updated

test "list predicates use structural equality"
  assert
    =
      list true true false
      list
        list.has
          list
            list 1 2
            list 3
          list 1 2
        list.some
          fn (value)
            > value 3
          list 1 4 2
        list.every
          fn (value)
            < value 3
          list 1 2 3

test "list case supports empty exact and rest patterns"
  assert
    =
      list "empty" "single:7" "many:1:2:2"
      list
        classify-list
          (list)
        classify-list
          list 7
        classify-list
          list 1 2 3 4

test "list pipeline combines map filter and fold"
  assert
    = 56
      square-evens-total
        list 1 2 3 4 5 6

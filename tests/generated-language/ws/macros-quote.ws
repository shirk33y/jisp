def unless
  ~
    fn (condition then otherwise) `(if ,condition ,otherwise ,then)

def preserve-caller
  ~
    fn (expression) `(let (value 1) ,expression)

def pair
  ~
    fn (left right)
      quote
        list "left" "right"

test "portable runner expands quote and user macros before lowering"
  assert.equal
    obj "unless" "chosen"
      ... "hygiene" 42
      ... "quote" (list "left" "right")
    let (value 42)
      obj "unless" (unless false "chosen" "skipped")
        ... "hygiene" (preserve-caller value)
        ... "quote" (pair 1 2)

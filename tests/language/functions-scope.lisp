(def make-adder
  (fn (base)
    (fn (value)
      (+ base value))))

(def apply-twice
  (fn (f value)
    (f (f value))))

(def pack
  (fn (head ... tail)
    (list.prepend head tail)))

(def sum-list
  (fn (values)
    (list.fold
      (fn (total value)
        (+ total value))
      0
      values)))

(def fact
  (fn (n)
    (if (= n 0)
      1
      (* n (fact (- n 1))))))

(test "closures capture lexical bindings"
  (assert.equal
    18
    ((make-adder 11) 7)))

(test "higher order functions accept returned closures"
  (assert.equal
    14
    (apply-twice (make-adder 4) 6)))

(test "variadic functions bind remaining arguments as a list"
  (assert.equal
    (list 1 2 3 4)
    (pack 1 2 3 4)))

(test "recursive definitions work with typed modules"
  (assert.equal
    720
    (fact 6)))

(test "folded helper keeps accumulator and item types distinct"
  (assert.equal
    15
    (sum-list (list 1 2 3 4 5))))

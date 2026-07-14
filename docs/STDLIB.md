# Standard library

This is the complete public prelude installed by the type checker and
interpreter. There are no imports for these names: call them directly. The
interpreter implements every function below. Native Rust emission supports only
the narrower subset described in the [README](../README.md), and rejects the
rest instead of falling back to a universal dynamic value.

Native bigint values are emitted as `num_bigint::BigInt`. A crate using
`jisp_macros::lisp_file!` with bigint code must declare a compatible direct
`num-bigint = "0.4"` dependency so the generated Rust can name that concrete
type.

`A`, `B`, `E`, and `F` denote inferred type variables. `N` means one matching
numeric type: `int`, `bigint`, or `float`. `(... A)` means zero or more
arguments of type `A`. Callback positions such as `list.map` require the fixed
arity shown in the signature; a variadic function is not valid there.

Compiler forms such as `def`, `export`, `fn`, `let`, `if`, `case`, `use`,
`type`, `import`, `obj`, `list`, `str`, quoting, and field lookup (`.`) are not
stdlib functions. Their syntax is specified in [SPEC.md](SPEC.md).

## Constructors and equality

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `ok` | `(A) -> result<A, E>` | Constructs a successful result. | `(ok 42)` |
| `err` | `(E) -> result<A, E>` | Constructs an error result. | `(err "missing")` |
| `some` | `(A) -> option<A>` | Constructs an option containing a value. | `(some "Ada")` |
| `none` | `option<A>` | The empty option constructor. | `none` |
| `bigint` | `(str) -> bigint` | Parses a base-10 arbitrary-precision integer; invalid input is a runtime error. | `(bigint "9223372036854775808")` |
| `=` | `(A, A) -> bool` | Structural equality. Functions and opaque native handles are not comparable. | `(= (list 1 2) (list 1 2))` |

## Numeric operators and `math`

Numbers never coerce implicitly. Matching integer operations are checked and
division by zero is an error. `/` truncates integer and bigint results toward
zero; `//` and `%` use Euclidean division and remainder. See [Numbers in the
language specification](SPEC.md#numbers) for the full contract.

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `+` | `(N, N) -> N` | Adds matching numbers. | `(+ 20 22)` |
| `-` | `(N, N) -> N` | Subtracts matching numbers. | `(- 44 2)` |
| `*` | `(N, N) -> N` | Multiplies matching numbers. | `(* 6 7)` |
| `/` | `(N, N) -> N` | Divides matching numbers; integer/bigint division truncates toward zero. | `(/ 85 2)` |
| `//` | `(N, N) -> N` | Euclidean division; a float result is floored. | `(// -5 2)` |
| `%` | `(N, N) -> N` | Euclidean remainder. | `(% -5 2)` |
| `<` | `(N, N) -> bool` or `(str, str) -> bool` | Strict ordered comparison. | `(< 1 2)` |
| `>` | `(N, N) -> bool` or `(str, str) -> bool` | Strict ordered comparison. | `(> "z" "a")` |
| `<=` | `(N, N) -> bool` or `(str, str) -> bool` | Non-strict ordered comparison. | `(<= 2 2)` |
| `>=` | `(N, N) -> bool` or `(str, str) -> bool` | Non-strict ordered comparison. | `(>= 3 2)` |
| `math.abs` | `(N) -> N` | Absolute value for an integer, bigint, or float. | `(math.abs -42)` |
| `math.min` | `(N, N) -> N` | Smaller of two matching numbers. | `(math.min 9 4)` |
| `math.max` | `(N, N) -> N` | Larger of two matching numbers. | `(math.max 9 4)` |
| `math.pow` | `(int, int) -> int` or `(float, float) -> float` | Raises a value to a power; integer exponents must be non-negative. | `(math.pow 2 10)` |
| `math.sqrt` | `(float) -> float` | Square root; negative values follow `f64` and produce `NaN`. | `(math.sqrt 9.0)` |
| `math.floor` | `(float) -> float` | Rounds down while retaining `float`. | `(math.floor 2.9)` |
| `math.ceil` | `(float) -> float` | Rounds up while retaining `float`. | `(math.ceil 2.1)` |
| `math.round` | `(float) -> float` | Rounds to the nearest integral-valued float. | `(math.round 2.5)` |

## `str`

String lengths and slice positions count Unicode scalar values, not bytes.
`str.slice` returns an error result for negative or out-of-bounds bounds.

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `str.is` | `(A) -> bool` | Tests whether a value is a string. | `(str.is "Ada")` |
| `str.from` | `(A) -> str` | Converts a value to its display string. | `(str.from 42)` |
| `str.len` | `(str) -> int` | Counts Unicode scalar values. | `(str.len "Żółw")` |
| `str.cat` | `(... str) -> str` | Concatenates zero or more strings. | `(str.cat "Ji" "sp")` |
| `str.join` | `(str, list<str>) -> str` | Joins a list using the first argument as delimiter. | `(str.join ", " (list "Ada" "Lin"))` |
| `str.split` | `(str, str) -> list<str>` | Splits the first string on the delimiter. | `(str.split "a,b" ",")` |
| `str.trim` | `(str) -> str` | Removes leading and trailing Unicode whitespace. | `(str.trim "  Ada ")` |
| `str.upper` | `(str) -> str` | Converts to Unicode uppercase. | `(str.upper "Ada")` |
| `str.lower` | `(str) -> str` | Converts to Unicode lowercase. | `(str.lower "ADA")` |
| `str.has` | `(str, str) -> bool` | Tests whether the first string contains the second. | `(str.has "Jisp" "is")` |
| `str.starts` | `(str, str) -> bool` | Tests a prefix. | `(str.starts "Jisp" "Ji")` |
| `str.ends` | `(str, str) -> bool` | Tests a suffix. | `(str.ends "Jisp" "sp")` |
| `str.replace` | `(str, str, str) -> str` | Replaces every non-overlapping occurrence. | `(str.replace "a-b-a" "a" "x")` |
| `str.slice` | `(str, int, int) -> result<str, str>` | Returns the half-open range `[start, end)`, or `err`. | `(str.slice "Jisp" 1 3)` |

## `list`

Lists are immutable. Indexes are zero-based and slice ranges are half-open.
The lookup and slice functions return `result` values rather than trapping.

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `list.is` | `(A) -> bool` | Tests whether a value is a list. | `(list.is (list 1 2))` |
| `list.len` | `(list<A>) -> int` | Returns the item count. | `(list.len (list "a" "b"))` |
| `list.get` | `(list<A>, int) -> result<A, str>` | Gets an item, or returns `err` for an invalid index. | `(list.get (list 10 20) 1)` |
| `list.first` | `(list<A>) -> result<A, str>` | Gets the first item, or `err` for an empty list. | `(list.first (list 10 20))` |
| `list.last` | `(list<A>) -> result<A, str>` | Gets the last item, or `err` for an empty list. | `(list.last (list 10 20))` |
| `list.rest` | `(list<A>) -> list<A>` | Returns all items after the first; empty input stays empty. | `(list.rest (list 10 20))` |
| `list.slice` | `(list<A>, int, int) -> result<list<A>, str>` | Returns a half-open slice, or `err` for invalid bounds. | `(list.slice (list 10 20 30) 1 3)` |
| `list.map` | `((A) -> B, list<A>) -> list<B>` | Applies a unary function to each item. | `(list.map (fn (x) (+ x 1)) (list 1 2))` |
| `list.filter` | `((A) -> bool, list<A>) -> list<A>` | Keeps items whose predicate result is true. | `(list.filter (fn (x) (> x 1)) (list 1 2))` |
| `list.fold` | `((B, A) -> B, B, list<A>) -> B` | Left fold; callback receives accumulator then item. | `(list.fold + 0 (list 1 2 3))` |
| `list.some` | `((A) -> bool, list<A>) -> bool` | True if any item satisfies the predicate. | `(list.some (fn (x) (= x 2)) (list 1 2))` |
| `list.every` | `((A) -> bool, list<A>) -> bool` | True if every item satisfies the predicate. | `(list.every (fn (x) (> x 0)) (list 1 2))` |
| `list.has` | `(list<A>, A) -> bool` | Tests structural membership. | `(list.has (list 1 2) 2)` |
| `list.cat` | `(... list<A>) -> list<A>` | Concatenates zero or more lists. | `(list.cat (list 1) (list 2 3))` |
| `list.prepend` | `(A, list<A>) -> list<A>` | Returns a list with an item at the front. | `(list.prepend 1 (list 2 3))` |
| `list.append` | `(list<A>, A) -> list<A>` | Returns a list with an item at the end. | `(list.append (list 1 2) 3)` |

## `obj`

Objects have string keys and immutable updates. In the interpreter, dynamic
keys are valid for all object helpers. Native Rust generation currently needs a
closed object shape. Dynamic `.`/`obj.get`/`obj.has` reads and `obj.set` are
supported when every field has the same concrete type. Convert a homogeneous
closed object with `obj.to-map` when runtime-sized updates such as dynamic
delete should use the explicit map ABI. Heterogeneous dynamic reads are type
errors unless the key is statically known; direct dynamic object deletion and
open rows remain interpreter-only.

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `obj.is` | `(A) -> bool` | Tests whether a value is an object. | `(obj.is (obj "name" "Ada"))` |
| `obj.len` | `(obj) -> int` | Counts keys. | `(obj.len (obj "name" "Ada"))` |
| `obj.has` | `(obj, str) -> bool` | Tests whether a key exists. | `(obj.has (obj "name" "Ada") "name")` |
| `obj.get` | `(obj, str) -> result<A, str>` | Looks up a key, returning `err` when absent. | `(obj.get (obj "name" "Ada") "name")` |
| `obj.set` | `(obj, str, A) -> obj` | Returns a copy with a key inserted or replaced. | `(obj.set (obj "name" "Ada") "name" "Lin")` |
| `obj.del` | `(obj, str) -> obj` | Returns a copy without a key; an absent key changes nothing. | `(obj.del (obj "name" "Ada") "name")` |
| `obj.keys` | `(obj) -> list<str>` | Returns keys in insertion order. | `(obj.keys (obj "name" "Ada" "age" 42))` |
| `obj.values` | `(obj) -> list<A>` | Returns values in insertion order; a closed static row must be homogeneous. | `(obj.values (obj "a" 1 "b" 2))` |
| `obj.to-map` | `(obj) -> map<str, A>` | Converts a homogeneous closed object to an explicit runtime-sized map. | `(obj.to-map (obj "a" 1 "b" 2))` |
| `obj.cat` | `(... obj) -> obj` | Merges left to right; later duplicate keys win. | `(obj.cat (obj "a" 1) (obj "b" 2))` |

## `map`

Maps are explicit homogeneous dictionaries with string keys and values of one
type `A`. They use the source type `(map str A)` and native Rust emits
`indexmap::IndexMap<String, A>`, not a dynamic `Value`. Duplicate keys keep the
last value. Updates return a new map value semantically; generated native code
may reuse an owned temporary when aliases cannot observe mutation.

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `map` | `(... str A) -> map<str, A>` | Builds a map from alternating key/value expressions. | `(map "a" 1 "b" 2)` |
| `map.len` | `(map<str, A>) -> int` | Counts keys. | `(map.len (map "a" 1))` |
| `map.has` | `(map<str, A>, str) -> bool` | Tests whether a key exists. | `(map.has (map "a" 1) "a")` |
| `map.get` | `(map<str, A>, str) -> result<A, str>` | Looks up a key, returning `err` when absent. | `(map.get (map "a" 1) "a")` |
| `map.set` | `(map<str, A>, str, A) -> map<str, A>` | Returns a map with a key inserted or replaced. | `(map.set (map "a" 1) "b" 2)` |
| `map.del` | `(map<str, A>, str) -> map<str, A>` | Returns a map without a key; an absent key changes nothing. | `(map.del (map "a" 1) "a")` |
| `map.keys` | `(map<str, A>) -> list<str>` | Returns keys in insertion order. | `(map.keys (map "a" 1 "b" 2))` |
| `map.values` | `(map<str, A>) -> list<A>` | Returns values in insertion order. | `(map.values (map "a" 1 "b" 2))` |
| `map.cat` | `(... map<str, A>) -> map<str, A>` | Merges left to right; later duplicate keys win. | `(map.cat (map "a" 1) (map "b" 2))` |

## `result` and `option`

`result` helpers preserve an `err` unless their contract explicitly recovers
from it. `option` deliberately has only its constructors; consume it with
`case`.

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `result.try` | `(result<A, E>, (A) -> result<B, E>) -> result<B, E>` | Calls the callback for `ok`, otherwise preserves `err`; `use` expands around this shape. | `(result.try (ok 2) (fn (x) (ok (+ x 1))))` |
| `result.map` | `(result<A, E>, (A) -> B) -> result<B, E>` | Maps the success value only. | `(result.map (ok 2) (fn (x) (+ x 1)))` |
| `result.map-err` | `(result<A, E>, (E) -> F) -> result<A, F>` | Maps the error value only. | `(result.map-err (err "bad") (fn (e) (str.cat "error: " e)))` |
| `result.recover` | `(result<A, E>, (E) -> result<A, F>) -> result<A, F>` | Calls the callback for `err`, otherwise preserves `ok`. | `(result.recover (err "bad") (fn (_) (ok 0)))` |

## `ui` and `io`

| Function | Signature | Description | Example |
| --- | --- | --- | --- |
| `ui.html` | `(A) -> str` | Static HTML renderer for a UI node; escapes text and attributes, flattens child lists, and ignores event/key metadata. See [UI.md](UI.md). | `(ui.html (save-button))` |
| `ui.node` | `(A) -> ui.node` | Internal identity marker emitted by the UI lowerer so heterogeneous UI children share one renderer-neutral type. UI source should use explicit elements and `text`, not call this directly. | `(button (text "Save"))` |
| `ui.action` | `(str, list<A>) -> ui.action-template` | Declares a portable action variant with static JSON-shaped fields. | `(ui.action "Saved" (list 42))` |
| `ui.action-result` | `(str, list<A>) -> ui.action-template` | Like `ui.action`, then appends the successful capability result as the final action field. | `(ui.action-result "Loaded" (list))` |
| `ui.action-error` | `(str, list<A>) -> ui.action-template` | Like `ui.action`, then appends a JSON `{code, message}` host error as the final action field. | `(ui.action-error "LoadFailed" (list))` |
| `ui.command` | `(str, str, int, A, bool, ui.action-template, ui.action-template) -> ui.command` | Creates a versioned command declaration: `(id capability version request replace on-ok on-error)`. Completion templates are data, never callbacks. | `(ui.command "save:1" "storage.write" 1 (obj "key" "draft") true (ui.action "Saved" (list)) (ui.action-error "SaveFailed" (list)))` |
| `ui.subscription` | `(str, str, int, A, bool, ui.action-template, ui.action-template) -> ui.subscription` | Creates a versioned subscription declaration with the same completion-template order as `ui.command`. | `(ui.subscription "clock" "timer.tick" 1 (obj "every-ms" 1000) false (ui.action-result "Tick" (list)) (ui.action-error "ClockFailed" (list)))` |
| `ui.result` | `(A, list<ui.command>, list<ui.subscription>) -> ui.update-result<A>` | Explicit reducer result carrying next state and declarative resources; it does not execute effects and is invalid from a view. | `(ui.result next-state (list command) (list subscription))` |
| `io.println` | `(A) -> null` | Writes a display value followed by a newline. | `(io.println "Hello")` |

## Runnable examples

These longer examples exercise representative API contracts in the normal test
suite. The per-function examples above are concise call-site references.

```lisp test=stdlib.list-pipeline mode=run
(export main
  (fn ()
    (list.fold
      (fn (total value) (+ total value))
      0
      (list.map
        (fn (value) (+ value 1))
        (list 1 2 3)))))
```

```lisp test=stdlib.result-use mode=run
(def parse-count
  (fn (raw)
    (case raw
      ("two" (ok 2))
      (_ (err "bad count")))))

(export main
  (fn ()
    (use value (result.try (parse-count "two"))
      (ok (+ value 1)))))
```

```lisp test=stdlib.variadic mode=run
(def count-rest
  (fn (first ... rest)
    (+ first (list.len rest))))

(export main
  (fn () (count-rest 40 1 2)))
```

```lisp test=stdlib.ui-html mode=run
(component save-button ()
  (button
    (attr "title" "Save <draft>")
    (class "px-4")
    (span (text "Save & close"))))

(export main
  (fn ()
    (ui.html (save-button))))
```

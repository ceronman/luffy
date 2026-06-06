# A Tour of Luffy

Luffy is a small, statically-typed language that compiles to WebAssembly. This tour walks through its core features.

---

## Hello, World

Luffy programs are collections of functions. There is no special entry point syntax — you declare a function and export it so the host can call it.

```luffy
import fn print_int(x Int)

export fn main() {
    print_int(42)
}
```

`import fn` brings in a function from the host environment. `export fn` makes a function callable from outside the WebAssembly module. The host is responsible for providing the imported functions and calling the exports.

---

## Types

Luffy has three primitive types:

| Type    | Description              | Example       |
|---------|--------------------------|---------------|
| `Int`   | 64-bit signed integer    | `42`, `-7`    |
| `Float` | 64-bit floating-point    | `3.14`, `-1.5`|
| `Bool`  | Boolean                  | `true`, `false`|

Types are always written with a capital letter. There are no implicit conversions between types — `Int` and `Float` are distinct.

---

## Variables

Declare a variable with `let`. The type annotation is required, and an initializer must be provided.

```luffy
let x Int = 10
let pi Float = 3.14
let flag Bool = true
```

Reassign a variable with `=`:

```luffy
let x Int = 1
x = 2
```

Variables are scoped to the block they are declared in. Declaring the same name twice in the same scope is an error.

---

## Arithmetic

The standard arithmetic operators work on `Int` and `Float`.

```luffy
let a Int = 2 + 3    // 5
let b Int = 10 - 4   // 6
let c Int = 3 * 7    // 21
let d Int = 20 / 4   // 5
let e Int = 17 % 5   // 2  (modulo, Int only)
```

Operator precedence follows the usual rules: `*`, `/`, `%` bind tighter than `+` and `-`. Use parentheses to control grouping:

```luffy
let a Int = 2 + 3 * 4    // 14
let b Int = (2 + 3) * 4  // 20
```

The unary `-` negates a numeric value:

```luffy
let n Int = -42
let f Float = -1.5
```

`%` is only defined for `Int`. Mixing `Int` and `Float` in the same expression is a type error.

---

## Comparison

Comparison operators return `Bool`:

```luffy
1 == 1    // true
1 != 2    // true
3 < 5     // true
5 <= 5    // true
7 > 4     // true
6 >= 6    // true
```

`==` and `!=` work on any type as long as both sides have the same type. The ordering operators (`<`, `<=`, `>`, `>=`) require numeric operands.

---

## Logic

Boolean operators are words, not symbols:

```luffy
true and false   // false
false or true    // true
not true         // false
```

`and` and `or` short-circuit: the right side is only evaluated if the left side does not determine the result. `and` returns `false` immediately if the left is `false`; `or` returns `true` immediately if the left is `true`.

`not` has higher precedence than `and` and `or`:

```luffy
not true or false    // (not true) or false  →  false
not (true or false)  // not true             →  false
```

---

## Functions

Functions are declared with `fn`. Parameters have the form `name Type`. The return type follows the closing parenthesis; omit it for functions that return nothing.

```luffy
fn add(a Int, b Int) Int {
    return a + b
}

fn greet() {
    print_int(1)
}
```

Call a function by name:

```luffy
let result Int = add(3, 4)
```

### Short function bodies

A function whose body is a single expression can be written with a colon (`:`) instead of a braces block. The expression after the colon is the function's return value, so no `return` is needed:

```luffy
fn add(a Int, b Int) Int: a + b
```

This is exactly equivalent to the braces form:

```luffy
fn add(a Int, b Int) Int {
    return a + b
}
```

The colon form accepts a *single expression only* — statements such as `let` declarations or assignments are not allowed after the colon. When you need more than one statement, use a braces block. As a matter of style, prefer the colon form for simple, single-expression functions and the braces form when the body does real work.

The function body must be either a braces block or a colon expression; a bare statement is no longer a valid function body.

Functions can call other functions defined anywhere in the file — declaration order does not matter. Recursion works too.

```luffy
fn square(x Int) Int {
    return x * x
}

fn sum_of_squares(a Int, b Int) Int {
    return square(a) + square(b)
}
```

---

## Imports and Exports

Luffy modules live inside a WebAssembly module. Functions from the host environment are brought in with `import fn` and made available to the rest of the module. No body is needed — the host provides the implementation.

```luffy
import fn print_int(x Int)
import fn print_float(x Float)
import fn print_bool(x Bool)
```

Functions marked `export fn` are callable from the host after instantiation. An exported function is also a regular function within the module and can be called by other Luffy functions.

```luffy
export fn add(a Int, b Int) Int {
    return a + b
}
```

A complete program that imports printing helpers and exports a `main` function:

```luffy
import fn print_int(x Int)
import fn print_bool(x Bool)

fn is_even(n Int) Bool {
    return n % 2 == 0
}

export fn main() {
    print_int(7)
    print_bool(is_even(4))
    print_bool(is_even(7))
}
```

---

## Comments

Line comments start with `//`:

```luffy
let x Int = 1  // this is a comment
```

Block comments are delimited by `/*` and `*/` and can be nested:

```luffy
/* This is
   a block comment */

/* outer /* inner */ still outer */
```

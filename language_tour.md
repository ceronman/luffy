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

## Statements and lines

Luffy has no mandatory statement terminator. Statements are separated by newlines, so the usual style is one statement per line:

```luffy
let x Int = 1
let y Int = 2
x = x + y
```

If you want to put several statements on a single line, separate them with a semicolon (`;`):

```luffy
let x Int = 1; let y Int = 2; x = x + y
```

Semicolons are only for statements inside a function body. Top-level items — functions and imports — need no separator at all, since each begins with a keyword. You can write them one per line or several on the same line; a `;` between items is an error:

```luffy
import fn print_int(x Int)
import fn print_bool(x Bool)

fn a() {} fn b() {}
```

Blank lines are ignored, and stray, repeated, or trailing semicolons between statements are harmless, so you can space code out however you like:

```luffy
fn main() {

    let x Int = 1;;

    print_int(x);
}
```

Two statements on the same line *without* a separator are an error — add a newline or a `;`:

```luffy
let x Int = 1 let y Int = 2   // error: expected newline or `;`
```

### Spanning multiple lines

Newlines are ignored inside parentheses and argument lists, and when an expression is clearly unfinished at the end of a line (for example, a line ending in an operator). This lets you wrap long signatures, calls, and expressions:

```luffy
fn add(
    a Int,
    b Int
) Int {
    return a +
        b
}

let total Int = add(
    1,
    2
)
```

Two cases are worth remembering:

- A binary operator at the *start* of the next line still continues the expression. `a` followed by `- b` on the next line is the subtraction `a - b`, not two statements. To write two statements, separate them explicitly.
- A `(` at the *start* of a new line begins a new statement rather than a function call. Keep the opening parenthesis on the same line as the function name:

```luffy
foo(x)     // a call

foo
(x)        // two statements: `foo`, then `(x)`
```

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

## Conditionals

`if` chooses between two branches based on a `Bool` condition. The condition is *not* parenthesized — like Rust, and unlike Java or Kotlin:

```luffy
if a > 0 {
    print_int(1)
} else {
    print_int(2)
}
```

Each branch is a block, so it follows the same two forms as a function body. The braces form holds any number of statements; the colon form holds a single expression:

```luffy
if a > 0: print_int(1) else: print_int(2)
```

### `if` as an expression

`if` is an expression: it produces a value, which means there is no separate ternary operator. The value of a branch is its last expression — for a colon branch that is the expression after the colon, and for a braces branch it is the value of the final expression statement.

```luffy
let a Int = if x > 0 { 1 } else { 2 }
let b Int = if y > 0: 1 else: 2
```

When `if` is used as an expression, both branches must be present and must have the same type, which becomes the type of the whole expression. A braces branch can do work before producing its value:

```luffy
let label Int = if score > 50 {
    print_int(score)
    1
} else {
    0
}
```

### Omitting `else`

When `if` is used as a statement, the `else` branch is optional and any branch value is discarded:

```luffy
if x > 0 {
    print_int(1)
}
```

But when `if` is used as an expression, the `else` branch is mandatory — there would otherwise be no value when the condition is false:

```luffy
let x Int = if a > 0: 0   // error: 'if' must have both main and 'else' branches when used as an expression
```

This mirrors how `if` works in Kotlin.

---

## Loops

`while` repeats a block as long as its condition is `true`. As with `if`, the condition is *not* parenthesized:

```luffy
while a > 0 {
    print_int(a)
    a = a - 1
}
```

The body follows the same two forms as a function body or an `if` branch. The braces form holds any number of statements; the colon form holds a single expression:

```luffy
while a > 0: print_int(a)
```

Note that the colon form's body is a single *expression*. Since assignment is a statement, not an expression, the common pattern of mutating a counter (`a = a - 1`) needs the braces form — the colon form is best reserved for side-effecting calls.

Unlike `if`, `while` is **not** an expression: it never produces a value, so it cannot be used where a value is expected. This is illegal:

```luffy
let a Unit = while a > 0: print_int(a)   // error: `while` is a statement, not an expression
```

### `break` and `continue`

Inside a loop, `break` stops the loop immediately, and `continue` skips the rest of the current iteration and re-tests the condition:

```luffy
let i Int = 0
while i < 10 {
    i = i + 1
    if i == 3: continue   // skip printing 3
    if i > 6: break       // stop once past 6
    print_int(i)
}
```

Both are *expressions* of type `Unit`, so they can appear anywhere an expression can — including a colon branch as above. Because they yield `Unit`, they cannot stand in for a real value (for example, as both arms of a value-producing `if`). Using `break` or `continue` outside a loop is a compile error.

Note the usual `continue` caveat: make sure the loop still makes progress before the `continue`, or it will spin forever. Above, `i` is incremented before any `continue`.

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

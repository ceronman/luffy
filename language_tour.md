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

Luffy has four primitive types:

| Type    | Description              | Example       |
|---------|--------------------------|---------------|
| `Int`   | 64-bit signed integer    | `42`, `-7`    |
| `Float` | 64-bit floating-point    | `3.14`, `-1.5`|
| `Bool`  | Boolean                  | `true`, `false`|
| `Byte`  | 8-bit unsigned integer   | `65` (in `Byte` context) |

`Byte` has no literal syntax of its own — an integer literal becomes a `Byte` when it appears where a `Byte` is expected. See the [Bytes](#bytes) section below.

Types are always written with a capital letter. There are no implicit conversions between types — `Int` and `Float` are distinct.

A type annotation is just a name. The compiler accepts any identifier where a type is expected and only checks it during type analysis, so an unrecognized type name is reported as a type error rather than a syntax error:

```luffy
let x Color = 1   // error: Unknown type
```

Besides the three primitives, two further types have names you can write but no literal values: `Unit` (the "no value" type of functions that return nothing) and `Never` (the type of expressions like `return` that never produce a value — see the section on `return` below).

Luffy also has one compound type, `Array[T]`, a fixed-size array of elements of type `T`. It is covered in the [Arrays](#arrays) section below.

### Integer literals

`Int` literals can be written in four bases. The `0x`/`0o`/`0b` prefixes and the hexadecimal digits `a`–`f` are case-insensitive:

```luffy
let dec Int = 42       // decimal
let hex Int = 0xFF     // hexadecimal — 255
let oct Int = 0o17     // octal       — 15
let bin Int = 0b101    // binary      — 5
```

An underscore may be used as a separator between digits to group long numbers. It is purely cosmetic and may appear in any base (and in float literals too):

```luffy
let million Int = 1_000_000
let mask    Int = 0xDE_AD_BE_EF
let flags   Int = 0b1010_0101
let pi      Float = 3.141_592
```

Underscores must sit *between* two digits. A leading, trailing, or doubled underscore (`_1`, `1_`, `1__0`), an underscore next to the base prefix (`0x_FF`), and digits that are illegal for the chosen base (`0o18`, `0xGG`) are all errors.

### Float literals

`Float` literals are always decimal and follow the same format as Python. A literal is a float if it has a decimal point, an exponent, or both:

```luffy
let a Float = 3.14
let b Float = 10.        // digits before the dot only
let c Float = .5         // digits after the dot only
let d Float = 1e10       // scientific notation
let e Float = 1.5e-3     // exponent with a sign
let f Float = 6.022e23   // mantissa and exponent
```

The exponent marker is `e` or `E`, optionally followed by a `+` or `-` sign, and then one or more digits. At least one digit must appear on some side of the decimal point, so a bare `.` is not a number.

`_` separators may appear between digits in the integer, fractional, and exponent parts, exactly as for integers:

```luffy
let big Float = 1_000.000_5
let avo Float = 6.022_140_76e23
```

---

## Variables

Declare a variable with `let`. The type annotation is required; the initializer is optional.

```luffy
let x Int = 10
let pi Float = 3.14
let flag Bool = true
```

You may omit the initializer to declare a binding and assign it later. Reading a variable before it has been assigned is undefined, so assign it before use:

```luffy
let x Int       // declared, not yet initialized
x = 7           // assigned here
```

Reassign a variable with `=`:

```luffy
let x Int = 1
x = 2
```

Assignment is an **expression** whose value is always `Unit` (like Rust). Because it is an expression, it can be used anywhere an expression is expected — most usefully as the single expression of a colon body:

```luffy
while a > 0: a = a - 1
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

But when `if` is used as an expression, the `else` branch is mandatory — there would otherwise be no value when the condition is false. A missing `else` is treated as producing `Unit`, so omitting it reports a type mismatch against the then-branch's type:

```luffy
let x Int = if a > 0: 0   // error: Type mismatch: expected 'Int', found 'Unit'
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

Both are *expressions* of type `Never` (see below), so they can appear anywhere an expression can — including a colon branch as above. Using `break` or `continue` outside a loop is a compile error.

Note the usual `continue` caveat: make sure the loop still makes progress before the `continue`, or it will spin forever. Above, `i` is incremented before any `continue`.

---

## Arrays

`Array[T]` is a fixed-size sequence of values that all share the element type `T`. The element type is written as a bracketed argument, and it can be any type — including another array, giving you nested arrays:

```luffy
let xs Array[Int]          // an array of Int
let grid Array[Array[Int]] // an array of arrays of Int
```

### Creating arrays

An array value is written as a comma-separated list of expressions between square brackets. This is a *collection literal*. Its element type is taken from the type it is assigned to, so the binding (or parameter, or return) must be annotated:

```luffy
let xs Array[Int] = [1, 2, 3]
let ys Array[Float] = [1.5, 2.5]
let empty Array[Int] = []          // an empty array is allowed
```

All elements must match the declared element type. Mixing types is an error:

```luffy
let xs Array[Int] = [1, 2.0]   // error: expected 'Int', found 'Float'
```

A bare collection literal with no expected type cannot be inferred, so `[1, 2, 3]` on its own (with nothing telling the compiler what kind of array it is) is an error. Give it a target type via a `let`, a parameter, a return type, or a call argument.

### Indexing

Read an element with `arr[i]`. The index is an `Int` expression and the result has the element type:

```luffy
let xs Array[Int] = [10, 20, 30]
let first Int = xs[0]
let n Int = xs[1 + 1]   // the index can be any Int expression
```

Indexing binds tighter than arithmetic, so `xs[0] + 1` reads `xs[0]` and then adds `1`. Indexing chains for nested arrays:

```luffy
let grid Array[Array[Int]] = [[1, 2], [3, 4]]
let x Int = grid[1][0]   // 3
```

Indexing a non-array, or using a non-`Int` index, is a type error. Indexing out of bounds is a runtime trap.

### Assigning through an index

An index expression can be the target of an assignment, which writes a new value into that slot. The assigned value must match the element type, and like every assignment the whole expression has type `Unit`:

```luffy
let xs Array[Int] = [1, 2, 3]
xs[1] = 99            // xs is now [1, 99, 3]

let grid Array[Array[Int]] = [[1, 2], [3, 4]]
grid[0][1] = 88       // write a single inner element
grid[1] = [7, 8]      // replace a whole inner array
```

### Arrays and functions

Arrays can be passed to and returned from functions like any other value:

```luffy
fn first(xs Array[Int]) Int: xs[0]

fn make() Array[Int] {
    return [1, 2, 3]
}
```

Putting it together — building an array in a loop and summing it:

```luffy
import fn print_int(x Int)

export fn main() {
    let xs Array[Int] = [0, 0, 0, 0, 0]
    let i Int = 0
    while i < 5 {
        xs[i] = i * i
        i = i + 1
    }

    let total Int = 0
    i = 0
    while i < 5 {
        total = total + xs[i]
        i = i + 1
    }
    print_int(total)   // 0 + 1 + 4 + 9 + 16 = 30
}
```

> Note: arrays are compared by reference identity only at the IR level, and equality (`==`/`!=`) on arrays is not currently supported by the code generator. Compare individual elements instead.

---

## Bytes

`Byte` is an 8-bit unsigned integer type with values from 0 to 255. It exists chiefly as the element type of byte arrays (and, in a coming phase, as the building block of strings).

`Byte` has no literal syntax of its own. Instead, an integer literal becomes a `Byte` when it appears where a `Byte` is expected — a `let` with a `Byte` annotation, an argument for a `Byte` parameter, an element of an `Array[Byte]` literal, a `Byte` struct field, an assignment to a `Byte` target, or a `Byte` return value:

```luffy
let b Byte = 65                      // literal inferred as Byte
let xs Array[Byte] = [72, 105, 33]   // elements inferred as Byte
xs[0] = 200                          // assigned literal inferred as Byte
```

The literal is range-checked at compile time, so `let b Byte = 256` is an error. Negation is not part of a literal, so `let b Byte = -1` is also an error: `-1` is the negation of the `Int` literal `1`, and negation produces an `Int`.

Bytes support equality and ordering comparisons — unsigned, so `200 > 100` holds even though 200 does not fit in a signed byte — and nothing else. There is **no `Byte` arithmetic**; do byte math in `Int` and convert back (`int_to_byte(byte_to_int(a) + byte_to_int(b))`):

```luffy
fn max_byte(a Byte, b Byte) Byte {
    if a < b {
        return b
    }
    return a
}

let sum Byte = a + b   // error: Operator requires numeric type
```

Only literals adapt to an expected type; everywhere else there is no implicit conversion between `Int` and `Byte`. In particular, the operands of a comparison have no expected type, so comparing a `Byte` directly against an integer literal is a type error — bind the literal to a `Byte` variable first, or convert it:

```luffy
let b Byte = 65
let bad Bool = b == 65               // error: expected 'Byte', found 'Int'

let sixty_five Byte = 65
let ok Bool = b == sixty_five
let also_ok Bool = b == int_to_byte(65)
```

`Byte` converts to and from `Int` with the builtins `byte_to_int(b)` (always succeeds) and `int_to_byte(n)` (traps at runtime unless `n` is in 0–255) — see [Builtin functions](#builtin-functions).

`Array[Byte]` is stored *packed*: each element occupies a single byte on the heap, unlike `Array[Int]` whose elements are eight bytes each. This is invisible in the language — indexing, assignment, and bounds behavior work like any other array.

---

## Builtin functions

Luffy pre-declares a set of builtin functions. They are called and type-checked like ordinary functions, but the compiler expands each call inline into one or a few Wasm instructions — there is no function call at runtime. Because they occupy the ordinary function namespace, declaring your own function with a builtin's name is an error ("already declared").

The set covers the Wasm numeric operations that have no Luffy operator, plus the `Byte`/`Int` conversions:

```luffy
import fn print_int(x Int)

export fn main() {
    print_int(int_popcnt(255))      // 8
    print_int(int_shr(-8, 1))       // -4
    print_int(float_to_int(2.9))    // 2
    let b Byte = int_to_byte(200)
    print_int(byte_to_int(b) + 1)   // 201
}
```

### Integer builtins

| Function | Result |
|----------|--------|
| `int_and(a Int, b Int) Int` | bitwise AND |
| `int_or(a Int, b Int) Int` | bitwise OR |
| `int_xor(a Int, b Int) Int` | bitwise XOR |
| `int_shl(a Int, n Int) Int` | shift left |
| `int_shr(a Int, n Int) Int` | arithmetic shift right (keeps the sign) |
| `int_shr_u(a Int, n Int) Int` | logical shift right (fills with zeros) |
| `int_rotl(a Int, n Int) Int` | rotate bits left |
| `int_rotr(a Int, n Int) Int` | rotate bits right |
| `int_clz(a Int) Int` | count leading zero bits (64 for 0) |
| `int_ctz(a Int) Int` | count trailing zero bits (64 for 0) |
| `int_popcnt(a Int) Int` | count one bits |
| `int_div_u(a Int, b Int) Int` | unsigned division |
| `int_rem_u(a Int, b Int) Int` | unsigned remainder |

Shift and rotate counts are taken modulo 64, following Wasm. `/` and `%` are the *signed* division and remainder; the `_u` variants reinterpret both operands as unsigned 64-bit values (so a negative operand looks huge). Like their signed counterparts, they trap when the divisor is zero.

### Float builtins

| Function | Result |
|----------|--------|
| `float_sqrt(x Float) Float` | square root |
| `float_abs(x Float) Float` | absolute value |
| `float_ceil(x Float) Float` | round up |
| `float_floor(x Float) Float` | round down |
| `float_trunc(x Float) Float` | round toward zero |
| `float_nearest(x Float) Float` | round to nearest, ties to even |
| `float_min(a Float, b Float) Float` | minimum |
| `float_max(a Float, b Float) Float` | maximum |
| `float_copysign(a Float, b Float) Float` | `a` with the sign of `b` |

### Conversions

| Function | Result |
|----------|--------|
| `int_to_float(n Int) Float` | exact for magnitudes below 2⁵³ |
| `float_to_int(x Float) Int` | truncates toward zero; **traps** on NaN or out-of-range values |
| `byte_to_int(b Byte) Int` | always succeeds |
| `int_to_byte(n Int) Byte` | **traps** unless `n` is in 0–255 |

---

## `return` and the `Never` type

`return` is also an expression. It hands a value back to the caller and stops executing the current function:

```luffy
fn classify(n Int) Int {
    if n > 0 {
        return 1
    }
    return 0
}
```

Because `return` never produces a value *to its surrounding expression* — control leaves the function instead — its type is `Never`. The same is true of `break` and `continue`, which transfer control out of (or around) a loop rather than yielding a value.

`Never` is the *bottom type*: the type with no values. It is compatible with every other type, so an expression that diverges can appear wherever any type is expected. This is what lets one branch of a value-producing `if` bail out while the other supplies the value:

```luffy
fn first_positive(a Int, b Int) Int {
    let x Int = if a > 0 { a } else { return b }   // else-branch is Never
    return x
}
```

Here the `else` branch has type `Never`, so the `if` takes the type of the other branch (`Int`). A non-`Unit` function must always end by diverging — either a trailing `return`, or an `if`/`else` in which every branch returns.

Other languages have the same idea under different names: Rust calls it `!` (the "never type"), Kotlin and Scala call it `Nothing`, TypeScript and Swift call it `Never`, and Haskell's empty type is `Void`. In every case it is the uninhabited type used to type expressions that never return normally.

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

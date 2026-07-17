# AGENTS.md — Luffy Compiler

This file documents the Luffy compiler for LLMs working with the source code.

---

## What Luffy Is

Luffy is an in-progress, statically-typed language that compiles to WebAssembly. It is implemented in Rust. The language is intentionally small; the compiler is a learning/research project. Do not assume features exist just because they are common in other languages — check the AST and parser before assuming.

---

## Build and Test

```bash
cargo build
cargo test
```

Tests use [insta](https://insta.rs/) for snapshot testing.

---

## Compiler Pipeline

Source text flows through five stages in sequence:

```
Source → Lexer → Parser → Semantic Analysis → Compiler → Emitter → Wasm binary
```

| Stage              | File                   | Input             | Output                   |
|--------------------|------------------------|-------------------|--------------------------|
| Lexer              | `src/lexer.rs`         | `&str`            | `Token` stream           |
| Parser             | `src/parser.rs`        | `Token` stream    | `ast::Module`            |
| Semantic Analysis  | `src/semantic.rs`      | `ast::Module`     | `semantic::Semantics`    |
| Compiler           | `src/compiler.rs`      | AST + Semantics   | `ir::Module`             |
| Emitter            | `src/emit.rs`          | `ir::Module`      | `Vec<u8>` (Wasm binary)  |

Each stage's public entry point:

- `lexer::Lexer::new(src).next_token()` / `next_significant()`
- `parser::parse(src) -> Result<ast::Module>`
- `semantic::semantic_analysis(&module) -> Result<Semantics>`
- `compiler::compile(&module, semantics) -> ir::Module`
- `emit::emit(module) -> Vec<u8>`

---

## Source File Map

```
src/
  ast.rs             — AST node types (canonical source of truth for what the language supports)
  lexer.rs           — Hand-written lexer; produces tokens including whitespace/comments
  parser.rs          — Recursive descent parser; consumes tokens, builds AST
  semantic.rs        — Name resolution and type checking; produces Semantics
  compiler.rs        — Lowers AST + Semantics to IR (ir::Module)
  ir.rs              — Wasm-like IR: types, instructions, imports, exports
  emit.rs            — Encodes ir::Module as a Wasm binary
  pretty.rs          — Error formatting and AST pretty-printing (used in tests)
  pretty/ast.rs      — AST pretty printer
  error.rs           — CompilerError type, ErrorKind enum
  main.rs            — CLI entry point

  lexer/test.rs      — Lexer unit tests
  parser/test.rs     — Parser snapshot tests
  semantic/test.rs   — Type-checker snapshot tests (valid programs + error cases)
  compiler/test.rs   — Compile-to-WAT snapshot tests
  emit/test.rs       — End-to-end run tests (compile + execute with wasmtime)
```

---

## AST Structure

`ast::Module` is the root. It contains a `Vec<Item>`.

### Items (top-level declarations)

```rust
enum ItemKind {
    Function { export: bool, name, params, return_ty: Option<TypeRef>, body: Block },
    Import   { name, params, return_ty: Option<TypeRef> },
}
```

There are only two kinds of top-level item. `import fn` has no body. `export fn` sets `export: true` on a `Function`.

### Function bodies (`Block`)

A function's `body` is a `Block`, not an arbitrary `Stmt`. A `Block` has two kinds:

```rust
struct Block { node: Node, kind: BlockKind }

enum BlockKind {
    Braces { statements: Vec<Stmt> },   // { ... }   — statements with explicit `return`
    Expr   { expr: Expr },              // : expr     — single expression, implicit return
}
```

- `BlockKind::Braces` is structurally identical to `StmtKind::Block` (a list of statements). Parsed by `Parser::braces_block`.
- `BlockKind::Expr` is the short colon form: `fn add(a Int, b Int) Int: a + b`. The expression is the implicit return value; its type must match the declared return type. Parsed by `Parser::expr_block`.

`Parser::function_body` dispatches on the first token (`{` → braces, `:` → expr); any other token is an error, so a bare statement is no longer a valid function body. Both the semantic pass (`Resolver::block`) and the compiler (`Compiler::block`) match on `BlockKind`: the `Expr` arm type-checks / lowers the expression and leaves its value on the stack as the function result (exactly like a trailing expression in a braces body).

`StmtKind::Block` is kept for future use (nested statement blocks under `if`/`while`).

### Statements

```rust
enum StmtKind {
    Declaration { name, ty: TypeRef, initializer: Option<Expr> },  // let x T [= expr]
    While       { condition: Expr, body: Box<Block> },             // while cond { ... } / while cond: expr
    ExprStmt    { expr: Expr },
}
```

The `let` **initializer is optional** (`let x Int` with no `= expr` is valid). When omitted, the binding reserves a slot but emits no store; a later assignment fills it. The declared type is still enforced for any later assignment.

**`if` is implemented** as an `ExprKind` variant (see Expressions below), not a `StmtKind`. **`while` is implemented** as a `StmtKind` variant (see Loops below). **`return`** is an `ExprKind` variant (it is an expression of type `Never`, see below), not a `StmtKind`. **Assignment** is also an `ExprKind` variant (an expression of type `Unit`, see below), not a `StmtKind`.

### Expressions

```rust
enum ExprKind {
    Literal  { kind: LiteralKind },                         // 1, 2.5, true
    Variable { name: Identifier },
    Collection { elements: Vec<Expr> },                     // [a, b, c]  (array literal)
    Unary    { op: UnOp, expr: Box<Expr> },                 // -x, not x
    Binary   { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Call     { callee: Box<Expr>, args: Vec<Expr> },
    Assignment { target: Box<Expr>, value: Box<Expr> },     // x = expr / arr[i] = expr  (always Unit)
    Index    { expr: Box<Expr>, index: Box<Expr> },         // arr[i]
    If       { condition: Box<Expr>, then_branch: Box<Block>, else_branch: Option<Box<Block>> },
    Break,                                                   // break    (Never; loops only)
    Continue,                                                // continue (Never; loops only)
    Return   { expr: Box<Expr> },                            // return e (Never)
}
```

Call expressions only work with a `Variable` callee at the compiler stage (function pointers are not supported yet). The semantic pass enforces the callee is a function type.

`Collection` is the array literal `[a, b, c]`, parsed when `[` appears in **prefix** position (`Parser::collection`). `Index` is `arr[i]`, parsed when `[` appears in **infix/postfix** position (`Parser::index`). Both are covered in detail in the "Arrays" section below. An `Index` may be the `target` of an `Assignment` (writing through a subscript); the semantic and compiler stages special-case `Variable` vs `Index` assignment targets.

`if` is an **expression**. Each branch is a `Block`, so it reuses the same braces/colon forms as a function body. The condition is parenthesis-free (Rust-style). A braces branch is typed by its last statement: if that statement is an expression statement, the branch's type is that expression's type; otherwise the branch is `Unit`. A colon branch is typed by its single expression.

`if` is handled by a single `ExprKind::If` arm in both `Resolver::expr` and `Compiler::expr` (there is no longer a separate `if_statement`/`if_expr` split):

- **Semantic** (`Resolver::expr`, `If` arm): the condition is checked via `check_condition`, both branches are typed via `block`, and `then_ty`/`else_ty` are unified. A missing `else` is treated as an `else` of type `Unit`, so the unification fails ("Type mismatch: expected `<then_ty>`, found `Unit`") unless the then-branch is itself `Unit`. The resulting type is the unified branch type (or the non-`Never` branch's type when one side diverges).
- **Compiler** (`Compiler::expr`, `If` arm): looks up the `if` node's type; a `Unit`/`Never` `if` lowers with `BlockType::Empty`, otherwise `BlockType::Result(T)` where each branch (`Compiler::branch`) leaves its value on the stack.

> **Behavioral note / known discrepancy:** because a missing `else` is modeled as `Unit`, an `if` *statement* whose then-branch produces a **non-`Unit`** value and has no `else` now fails to type-check (e.g. `if c { some_int_call() }` discarding the result). The older design (still described in the tour and below) intended statement-position `if` to discard the branch value and allow a missing `else` regardless of type. Decide whether to restore that behavior (re-introduce a statement-vs-expression distinction) or keep the current stricter rule.

### Loops (`while`)

`while` is a `StmtKind::While { condition, body }`, parsed by `Parser::while_statement` and dispatched from `Parser::statement` on `TokenKind::While`. The condition is parenthesis-free (like `if`) and the body reuses `Parser::block_body` (the same braces/colon forms as a function body or `if` branch).

`while` is a **statement only** — it is never wired into `expression_precedence`, so using it in expression position (e.g. `let a Int = while ...`) is a parse error ("Unexpected `while`"). It always has type `Unit`.

- **Semantic** (`Resolver::stmt`, `StmtKind::While` arm): checks the condition is `Bool` via the shared `check_condition(.., keyword)` helper (now parameterized so the error names `if` vs `while`), then type-checks the body with `block` (which opens/closes a scope) and discards its type.
- **Compiler** (`Compiler::stmt`, `StmtKind::While` arm): lowers to the standard Wasm `block` / `loop` / `br_if` / `br` shape:

  ```wat
  block            ;; break target
    loop           ;; continue target
      <condition>
      i32.eqz
      br_if 1      ;; condition false -> exit
      <body>
      br 0         ;; re-test condition
    end
  end
  ```

  The body is lowered keeping the operand stack balanced: a braces body runs each statement through `stmt` (expression statements already drop their non-`Unit` values), and a colon body is lowered via `expr_stmt`, which drops the value if non-`Unit`. The new IR instructions `Block`, `Loop`, `Br(LabelIdx)`, and `BrIf(LabelIdx)` are encoded in `emit.rs`.

#### `break` / `continue`

`break` and `continue` are `ExprKind::Break` / `ExprKind::Continue` — value-less expressions of type `Never` (see "The `Never` type" below), parsed as prefixes in `expression_precedence` (`Parser::break_expr` / `continue_expr`). They double as statements (an `ExprStmt`) and may appear in any expression slot, e.g. a colon `if` branch.

- **Semantic**: the `Resolver` tracks `loop_depth`, incremented around a `while` body. The `ExprKind::Break` / `Continue` arms in `Resolver::expr` give type `Never` and error ("'break' outside of a loop") when `loop_depth == 0`.
- **Compiler**: Wasm `br` takes a *relative* label index, so the compiler keeps a `LoopControl { depth, stack }` (`self.loops`) where `depth` counts open `block`/`loop`/`if` frames (bumped via `enter_frame`/`exit_frame` around the `If` arm and the `and`/`or` short-circuit `if`s, and `enter_loop`/`exit_loop` around the `While` arm's block+loop) and `stack` is a `VecDeque<LoopFrame>` of `{ break_target, continue_target }` frame indices. `break`/`continue` lower to `Br((depth - 1) - target)` (via `break_label`/`continue_label`), correct at any nesting depth (e.g. `break` from inside an `if` emits `br 2`). Because of this state, the body-lowering methods (`function`, `block`, `stmt`, `expr`, `branch`, `expr_stmt`) take `&mut self`; `function` calls `self.loops.reset()` per function.

#### `return` and `Never`

`return` is `ExprKind::Return { expr }`, a prefix expression (`Parser::return_expr`) of type `Never`. Its operand must match the enclosing function's return type (checked in `Resolver::expr`). It lowers (`Compiler::expr`) to the value followed by a Wasm `return` instruction, so it is correct in any position — including early returns inside `if` branches. Note this means a *tail* `return` also emits a (redundant but valid) `return` instruction.

`Never` (`semantic::Type::Never`) is the bottom type — the type of expressions that never yield a value because control leaves them (`return`/`break`/`continue`). Key rules:

- `unify_ty` succeeds whenever **either** side is `Never` (it is compatible with every type).
- In the `ExprKind::If` arm, when one branch is `Never` the whole `if` takes the *other* branch's type (the "join"), so `let x Int = if c { return 0 } else { 5 }` type-checks as `Int`.
- A braces function body with a non-`Unit` return type must have block type `Never` (it ends in a `return`, or an `if`/`else` where all branches `return`) — otherwise it's a "Missing return statement" error. (Replaces the old "last statement is literally a `return`" check, so `if`/`else`-return bodies now type-check.)
- In the compiler, an `if` whose node type is `Never` uses `BlockType::Empty` (no result), and `expr_stmt` does not emit a `drop` for `Never` (or `Unit`) values. `Never` is never passed to the type-lowering helpers (`Types::val_ty`/`wasm_ty`).

#### Assignment

`x = expr` is `ExprKind::Assignment { target, value }`, an **expression** whose type is always `Type::Unit` (like Rust). Because it is an expression, it may appear anywhere an expression is expected — in particular as the single expression of a colon body, so `while c: a = a + 1` works.

- **Parsing** (`Parser::assignment`, reached from `Parser::expression`): assignment binds looser than every binary operator and is **right-associative**, so `a = b = c` parses as `a = (b = c)`. `assignment` parses a full operator-precedence expression for the target, then — if an `=` follows — recurses for the value. The `=` continues across a newline like a binary operator. `expr_stmt` no longer special-cases `=`; it just wraps an expression.
- **Semantic** (`ExprKind::Assignment` arm in `Resolver::expr`): the target is type-checked, then its `kind` must be a `Variable` or an `Index` (otherwise "Invalid assignment target"). The value's type must unify with the target's type (the variable's type, or the indexed element type). The assignment node's type is `Unit`.
- **Compiler** (`ExprKind::Assignment` arm in `Compiler::expr`): a `Variable` target emits the value then `LocalSet(addr)`; an `Index` target emits the array, the index (wrapped to `i32` via `I32WrapI64`), the value, then `ArraySet(type_idx)`. Either way nothing is left on the operand stack (consistent with `Unit`), so `expr_stmt` emits no `drop`.

#### Arrays

`Array[T]` is the language's only compound type and is implemented end to end as a Wasm GC array. The three moving parts are the type (`Array[T]`), the literal (`ExprKind::Collection`), and indexing (`ExprKind::Index`, both reading and — as an assignment target — writing).

**Type syntax & resolution.** `Array[Int]` parses to a `TypeRef { name: "Array", args: [Int] }` (`Parser::type_ref` parses optional `[...]` generic arguments on any type name). `Resolver::ty_ref` maps `Array` with exactly one argument to `Type::Array { ty: Rc<Type> }`, resolving the element recursively (so `Array[Array[Int]]` works); zero or 2+ arguments give "Invalid array type". `Display` prints it as `Array[T]`.

**Collection literals** (`[a, b, c]`, `Parser::collection`, prefix `[`): there is **no array type literal** — a collection's element type is taken from the **expected type** threaded through `Resolver::expr`. The `ExprKind::Collection` arm requires an expected `Array[T]` (else "Not enough information to infer type of collection" or "expected `<T>`, found collection"), then unifies every element against `T`. An empty `[]` is allowed and takes the expected type. The expected type reaches the literal from: `let` initializers, `return`, the colon body, call arguments, and the trailing position of a block/branch. The compiler lowers a collection to: push each element, then `ArrayNewFixed { type_idx, len }`.

**Indexing** (`arr[i]`, `Parser::index`, infix `[`, precedence 7): `ExprKind::Index { expr, index }`. Semantic: `expr` must be `Array[T]` (else "expected Array, found ...") and `index` must be `Int` (else "index expected as Int, found ..."); the result type is `T`. Compiler read: push the array, push the index, `I32WrapI64` (Int is `i64`, the array opcode wants `i32`), `ArrayGet(type_idx)`. Writing through an index is handled by the `Assignment` arm (see above), emitting `ArraySet`. Indexing is left-associative and chains for nested arrays (`grid[i][j]`).

**IR / emit.** `ir::Type::Array { ty: StorageType }` is the defined array type; `ir::ValType::Ref(TypeIdx)` is a (nullable) reference to it. The instructions are `ArrayNewFixed { type_idx, len }`, `ArrayGet(type_idx)`, and `ArraySet(type_idx)`, encoded in `emit.rs` to the corresponding `wasm_encoder` `array.new_fixed` / `array.get` / `array.set`. The type section encodes arrays as `(array (mut T'))` (always mutable). Running requires the GC + function-references proposals (`Config::wasm_gc(true)`, `wasm_function_references(true)`).

**Known limitations / gaps (not yet handled):**

- `==` / `!=` on arrays type-checks (the `Binary` `Eq`/`Ne` arm returns `Bool` for any matching operand type) but the compiler's `Binary` arm has no array (or `Bool`) case, so lowering such a comparison hits `panic!("Unsupported binary operation")`. Equality on arrays should be rejected in semantics or lowered structurally. (The same latent gap means `Bool == Bool` currently panics in the compiler even though it type-checks.)
- Array length, iteration sugar, slicing, and bounds metadata do not exist; out-of-bounds access traps at runtime.

### Types

```rust
struct TypeRef { node: Node, name: Identifier, args: Vec<TypeRef> }   // in AST (syntax) — unresolved type name + generic args
enum Type {                                                          // in semantic (runtime)
    Unit, Int, Float, Bool, Byte, Never,
    Array { ty: Rc<Type> },
    Function { params: Rc<[Type]>, ret: Rc<Type> },
}
```

**Type names are resolved during semantic analysis, not parsing.** A `TypeRef` is the raw identifier the user wrote (e.g. `Int`) plus any bracketed generic arguments (e.g. `Array[Int]` → name `Array`, args `[Int]`), captured verbatim by `Parser::type_ref` — the parser accepts *any* identifier as a type, parses optional `[...]` arguments, and does no validation. `Resolver::ty_ref(&TypeRef) -> Result<Type>` maps the name to a `Type`, accepting `Int`, `Float`, `Bool`, `Byte`, `Unit`, `Never`, and `Array[T]` (exactly one argument, resolved recursively); any other name is a **Resolve error** (`"Unknown type"`), and a malformed `Array` (zero or more than one argument) is `"Invalid array type"`, reported at the type's span. This is why "unknown type"/"invalid array type" tests live in `semantic/test.rs`, not `parser/test.rs`. All annotation sites (function params and return type in `Resolver::module`/`function`, and `let` declarations in `Resolver::stmt`) go through `ty_ref`. There is no `TypeRef::lower`; that mapping now lives in `ty_ref`.

`Unit` is the type of functions with no return type annotation. It has no literal and cannot be stored in a variable, though it may now be written explicitly as a type name. `Never` is the bottom type of diverging expressions (`return`/`break`/`continue`); it has no literal, no Wasm representation, and is compatible with every type (see "The `Never` type" above).

### Type-to-Wasm mapping

| Luffy `Type` | Wasm `ValType`                                  |
|--------------|-------------------------------------------------|
| `Int`        | `i64`                                           |
| `Float`      | `f64`                                           |
| `Bool`       | `i32`                                           |
| `Byte`       | `i32` (unsigned, 0–255)                         |
| `Unit`       | (no result)                                     |
| `Array[T]`   | `(ref null $a)` where `$a = (array (mut T'))`   |

Arrays are Wasm GC arrays: each distinct `Array[T]` adds an `(array (mut T'))` entry to the type section (where `T'` is the lowered element `ValType`), and array *values* are nullable references to that type. `Array[Byte]` is the one *packed* array: it lowers to `(array (mut i8))`, element reads use `array.get_u` (plain `array.get` is invalid on packed arrays, and `Byte` is unsigned), and `Byte` comparisons compile to the unsigned `i32` comparison instructions. An integer literal types as `Byte` (compiling to `i32.const`) when its expected type is `Byte`, with a compile-time 0–255 range check; there are no other implicit `Int`↔`Byte` conversions. The runtime is configured with `wasm_gc(true)` and `wasm_function_references(true)` (see `emit::run` and the `emit`/test harness).

---

## Semantic Analysis Details

`semantic::Semantics` is produced by `semantic_analysis` and consumed by the compiler. It has three fields:

```rust
struct Semantics {
    expr_types:   HashMap<NodeId, Type>,          // type of every expression node
    declarations: Vec<Declaration>,               // all names declared, in order
    uses:         HashMap<NodeId, DeclarationId>, // identifier node → declaration
}
```

`DeclarationId` is an index into `semantics.declarations`.

`Declaration::kind` is `DeclarationKind::Function`, `DeclarationKind::Import`, `DeclarationKind::Struct`, `DeclarationKind::Builtin`, or `DeclarationKind::Local(fn_id)`, where `fn_id` is the `DeclarationId` of the enclosing function. The compiler uses `Local(fn_id)` to assign local variable indices per function, and distinguishes `Import` from `Function` so imports get the lowest Wasm function indices.

All functions (including imports) are pre-declared before any body is type-checked, so forward calls and mutual recursion work without special handling. Before that, `Resolver::declare_builtins` declares the builtin functions (`int_and`, `float_sqrt`, `int_to_byte`, … — see "Builtin Functions" below) in the root scope, so a user item reusing one of their names fails with "already declared".

---

## Compiler Details

`compiler::compile` does two passes over `module.items`:

1. **First pass:** assigns `FuncIdx` to every import and function in declaration order. Imports get the lowest indices (they appear first in the Wasm binary).
2. **Second pass:** emits function bodies for non-import items.

Local indices are assigned inline in `Compiler::module` (there is no separate `calculate_addresses()`): a pass over `semantics.declarations` gives each `DeclarationKind::Local(fn_id)` the next index for its function (via a per-`fn_id` counter). Parameters and locals share the same index space within a function (parameters, which are declared first, occupy indices 0..N, then locals follow). `Compiler::function` rebuilds the function's `locals` list by taking that function's `Local` declarations in order and skipping the first `params.len()`.

`and` and `or` compile to Wasm `if/else/end` blocks to implement short-circuit evaluation. They are handled specially in `Compiler::expr` before the standard left/right operand push pattern.

The import module name is hardcoded as `"js"`.

### Wasm types (`Compiler::Types`)

The compiler interns Wasm types in a `WasmTypes` helper keyed by the semantic `Type` (`unique_types: HashMap<Type, TypeIdx>`):

- `val_ty(&Type) -> ir::ValType` lowers a *value* type: `Int→i64`, `Float→f64`, `Bool→i32`, and `Array[T]→Ref(idx)` (creating the array type on demand). `Unit`/`Never`/`Function` are not value types (`todo!()`).
- `wasm_ty(&Type) -> ir::Type` builds a *defined* type for the type section: `Function` → `(func ...)`, `Array[T]` → `(array (mut T'))`. Element/param/result types are lowered with `val_ty`, so a nested `Array[Array[Int]]` creates the inner array type **before** the outer one (no forward references).
- `type_idx` dedupe by semantic `Type`, so two functions with the same signature — or two `Array[T]` with the same element type — share one type-section entry.

---

## Builtin Functions

Builtins are functions pre-declared by the compiler (`Resolver::declare_builtins` in `semantic.rs`, `DeclarationKind::Builtin`) and lowered inline: `Compiler::expr`'s `Call` arm dispatches to `Compiler::builtin_call`, which emits a fixed instruction sequence at the call site instead of a `call`. They type-check exactly like user functions (arity and argument types), and they exist so that Wasm numeric instructions without a Luffy operator are reachable from source code.

The current set (all signatures over `Int`/`Float`/`Byte`):

- **Int ↦ Wasm `i64`**: `int_and`, `int_or`, `int_xor`, `int_shl`, `int_shr` (`shr_s`), `int_shr_u`, `int_rotl`, `int_rotr`, `int_clz`, `int_ctz`, `int_popcnt`, `int_div_u`, `int_rem_u`.
- **Float ↦ Wasm `f64`**: `float_sqrt`, `float_abs`, `float_ceil`, `float_floor`, `float_trunc`, `float_nearest`, `float_min`, `float_max`, `float_copysign`.
- **Conversions**: `int_to_float` (`f64.convert_i64_s`), `float_to_int` (`i64.trunc_f64_s`, traps on NaN/out-of-range), `byte_to_int` (`i64.extend_i32_u`), `int_to_byte` (range-checked: traps via `unreachable` unless 0–255).

`int_to_byte` is the one multi-instruction lowering: it needs its argument twice, so it uses a per-function **i64 scratch local** (`Compiler::scratch_i64`), allocated lazily after params and user locals; `Compiler::function` appends it to the locals list when used. The single slot is safe to share between call sites because every inline sequence stores and consumes it without evaluating other expressions in between. The `if/unreachable` trap check opens a control frame, so the sequence wraps it in `loops.enter_frame()`/`exit_frame()` to keep `break`/`continue` label arithmetic correct.

To add a new inline builtin: add its signature to `declare_builtins`, its lowering arm to `builtin_call`, any new `ir::Instruction` variants + `emit.rs` encodings, and tests at the semantic/compiler/emit layers (`language_tour.md` has a user-facing table to extend).

---

## Newline and Statement Separation

Luffy has no mandatory statement terminator. Statements (inside a block) are
separated by **newlines** or by an explicit **`;`**; multiple statements on one
line require a `;`. Module **items** (functions, imports) need **no separator**
at all — each begins with a keyword, so they parse one after another whether on
separate lines or the same line. Newlines between items are optional, and a `;`
between items is a parse error.

The mechanism is Swift-style (`isAtStartOfLine`), not a newline token in the
parser's stream:

- The lexer still produces an `Eol` token from `next_token()`, but the parser
  (and the lexer tests) drive off `Lexer::next_significant()`, which skips
  whitespace, comments, **and** `Eol`, recording on the returned token whether
  a newline was skipped to reach it (`Token::newline_before`).
- Because the parser never sees `Eol` tokens, newlines are insignificant
  *everywhere by default* — inside parameter lists, call arguments, groupings,
  and partially-complete expressions (e.g. a trailing binary operator).
- Newlines become significant only where the parser explicitly asks:
  `Parser::expect_statement_separator` checks `newline_before` / `;` /
  end-of-block after each statement, and `skip_semicolons` consumes runs of `;`
  inside blocks. `module` requires no separator between items.
- Disambiguation: a `(` at the **start of a new line** does not continue the
  previous expression as a call (it begins a new statement). On the same line
  it is still a call. Binary operators, by contrast, always continue across a
  newline, so `a\n- b` is subtraction, not two statements.

**Requirement: any new syntax added to the language MUST include parser tests
for newline handling.** A new construct should be tested both formatted across
multiple lines and on a single line (with `;` where it separates statements),
asserting the two parse to the same AST, plus a missing-separator error case
where relevant. See the "Newline and semicolon handling" section in
`src/parser/test.rs` for the equivalence-based pattern to follow.

---

## Operator Precedence (high to low)

| Precedence | Operators                          |
|------------|------------------------------------|
| 8          | `.` (dot), `(` (function call)     |
| 7          | `[` (index), `not`, `-` (unary prefix) |
| 6          | `*`, `/`, `%`                      |
| 5          | `+`, `-`                           |
| 4          | `>`, `>=`, `<`, `<=`               |
| 3          | `==`, `!=`                         |
| 2          | `and`                              |
| 1          | `or`                               |

The parser uses Pratt parsing (`expression_precedence(min_precedence)`). A leading `[` in prefix position is a collection literal, not an index; in infix position it is an index (`a[i]`).

Note the newline rule for `[` differs from `(`: a `(` at the start of a new line begins a new statement (Swift-style disambiguation), but a `[` always continues the previous expression as an index, even across a newline. So `a\n[0]` parses as the single index `a[0]`.

---

## Error Handling

All stages return `Result<T, CompilerError>`. `CompilerError` contains:

```rust
struct CompilerError {
    kind: ErrorKind,   // Parse | Resolve | Type
    msg:  String,
    span: Span,        // byte range in source
}
```

`pretty::annotate_error_single` formats an error with a source-underline caret, used in all test snapshots.

---

## How to Add a New Feature

The typical sequence when adding a language feature (e.g. `if` expressions):

1. **Lexer** (`src/lexer.rs`): ensure the relevant token(s) exist. For `if`/`else` they already do.
2. **AST** (`src/ast.rs`): add the new variant to `StmtKind` or `ExprKind`.
3. **Parser** (`src/parser.rs`): add a parsing method and wire it into `statement()` or `expression_precedence()`.
4. **Semantic** (`src/semantic.rs`): add a match arm in `Resolver::stmt` or `Resolver::expr` to type-check the new construct and record types.
5. **Compiler** (`src/compiler.rs`): add a match arm in `Compiler::stmt` or `Compiler::expr` to emit IR instructions.
6. **IR** (`src/ir.rs`): add any new `Instruction` variants needed (e.g. `Block`, `BrIf`).
7. **Emitter** (`src/emit.rs`): encode the new instructions as Wasm bytes.
8. **Tests**: add snapshot tests at each layer (`parser/test.rs`, `semantic/test.rs`, `compiler/test.rs`) and an end-to-end execution test in `emit/test.rs`. **Always include newline-handling tests for any new syntax** — verify the construct parses identically across multiple lines and on a single line (using `;` where statements are separated), and that a missing separator is an error. See "Newline and Statement Separation" above.
9. **Documentation**: update the `language_tour.md` file and `AGENTS.md`

Each layer should be tested in isolation before moving to the next. The snapshot tests make regressions easy to catch.

---

## Test Conventions

- **Snapshot tests** (`assert_snapshot!`): the expected output is inlined after `@`. Never run `cargo insta accept` .
- `semantic/test.rs`: tests named `valid_*` expect `"<no error>"`. Tests named `error_*` expect a formatted error string with source underlines.
- `compiler/test.rs`: compiles to WAT (WebAssembly Text format) using `wasmprinter`. Check WAT output to verify correct instruction selection and local index assignment.
- `emit/test.rs`: actually executes the Wasm binary using `wasmtime`. The host provides `js.print_int`, `js.print_float`, and `js.print_bool`. The helper `run_main(body)` wraps a snippet in import declarations + `export fn main() { … }` automatically.

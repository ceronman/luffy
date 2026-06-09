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

Tests use [insta](https://insta.rs/) for snapshot testing. When adding a new test or changing compiler output, always leave
the actual output empty and prompt the user to review the tests using `cargo insta test && cargo insta review`.

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
    Declaration { name, ty: TypeRef, initializer: Expr },   // let x T = expr
    Assignment  { target: Expr, value: Expr },              // x = expr
    While       { condition: Expr, body: Box<Block> },      // while cond { ... } / while cond: expr
    ExprStmt    { expr: Expr },
}
```

**`if` is implemented** as an `ExprKind` variant (see Expressions below), not a `StmtKind`. **`while` is implemented** as a `StmtKind` variant (see Loops below). **`return`** is an `ExprKind` variant (it is an expression of type `Never`, see below), not a `StmtKind`.

### Expressions

```rust
enum ExprKind {
    Literal  { kind: LiteralKind },                         // 1, 2.5, true
    Variable { name: Identifier },
    Unary    { op: UnOp, expr: Box<Expr> },                 // -x, not x
    Binary   { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Call     { callee: Box<Expr>, args: Vec<Expr> },
    If       { condition: Box<Expr>, then_branch: Box<Block>, else_branch: Option<Box<Block>> },
    Break,                                                   // break    (Never; loops only)
    Continue,                                                // continue (Never; loops only)
    Return   { expr: Box<Expr> },                            // return e (Never)
}
```

Call expressions only work with a `Variable` callee at the compiler stage (function pointers are not supported yet). The semantic pass enforces the callee is a function type.

`if` is an **expression**. Each branch is a `Block`, so it reuses the same braces/colon forms as a function body (`Parser::branch_block` mirrors `Parser::function_body`). The condition is parenthesis-free (Rust-style). A braces branch is typed by its last statement: if that statement is an expression statement, the branch's type is that expression's type; otherwise the branch is `Unit`. A colon branch is typed by its single expression.

`if` behaves differently depending on position, and both the semantic pass and the compiler special-case it:

- **Statement position** (the `if` is the direct `expr` of a `StmtKind::ExprStmt`): the `else` branch is optional and the branch value is discarded. `Resolver::if_statement` / `Compiler::if_statement` handle this; the construct has type `Unit` and lowers to a Wasm `if` with `BlockType::Empty`, dropping any branch value in place (`Compiler::branch_discard`).
- **Expression position** (anywhere else — declaration initializer, `return`, call argument, operand, a branch's trailing expression, a colon function body): the `else` branch is **mandatory** and both branches must have the same type, which becomes the type of the `if`. Handled by the `ExprKind::If` arm in `Resolver::expr` / `Compiler::expr`; lowers to a Wasm `if` with `BlockType::Result(T)`, each branch leaving its value on the stack (`Compiler::branch`).

This statement-vs-expression `else` rule matches Kotlin. The `Resolver` tracks the enclosing function in a `current_function` field so branch resolution can reach the function's return type and declare branch-local variables.

### Loops (`while`)

`while` is a `StmtKind::While { condition, body }`, parsed by `Parser::while_statement` and dispatched from `Parser::statement` on `TokenKind::While`. The condition is parenthesis-free (like `if`) and the body reuses `Parser::block_body` (the same braces/colon forms as a function body or `if` branch).

`while` is a **statement only** — it is never wired into `expression_precedence`, so using it in expression position (e.g. `let a Int = while ...`) is a parse error ("Unexpected `while`"). It always has type `Unit`.

- **Semantic** (`Resolver::stmt`, `StmtKind::While` arm): checks the condition is `Bool` via the shared `check_condition(.., keyword)` helper (now parameterized so the error names `if` vs `while`), then type-checks the body with `block_type` (which opens/closes a scope) and discards its type.
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

- `check_ty_match` succeeds whenever **either** side is `Never` (it is compatible with every type).
- In `if_expr`, when one branch is `Never` the whole `if` takes the *other* branch's type (the "join"), so `let x Int = if c { return 0 } else { 5 }` type-checks as `Int`.
- A braces function body with a non-`Unit` return type must have block type `Never` (it ends in a `return`, or an `if`/`else` where all branches `return`) — otherwise it's a "Missing return statement" error. (Replaces the old "last statement is literally a `return`" check, so `if`/`else`-return bodies now type-check.)
- In the compiler, an `if` whose node type is `Never` uses `BlockType::Empty` (no result), and `expr_stmt` does not emit a `drop` for `Never` (or `Unit`) values. `Never` is never passed to `Type::lower()`.

### Types

```rust
enum TypeKind { Int, Float, Bool }              // in AST (syntax)
enum Type { Unit, Int, Float, Bool, Never, Function { params, ret } }  // in semantic (runtime)
```

`Unit` is the type of functions with no return type annotation. It has no literal and cannot be stored in a variable. `Never` is the bottom type of diverging expressions (`return`/`break`/`continue`); it has no literal, no Wasm representation, and is compatible with every type (see "The `Never` type" above).

### Type-to-Wasm mapping

| Luffy `Type` | Wasm `ValType` |
|--------------|----------------|
| `Int`        | `i64`          |
| `Float`      | `f64`          |
| `Bool`       | `i32`          |
| `Unit`       | (no result)    |

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

`Declaration::kind` is either `DeclarationKind::Function` or `DeclarationKind::Local(fn_id)`, where `fn_id` is the `DeclarationId` of the enclosing function. The compiler uses this to assign local variable indices per function.

All functions (including imports) are pre-declared before any body is type-checked, so forward calls and mutual recursion work without special handling.

---

## Compiler Details

`compiler::compile` does two passes over `module.items`:

1. **First pass:** assigns `FuncIdx` to every import and function in declaration order. Imports get the lowest indices (they appear first in the Wasm binary).
2. **Second pass:** emits function bodies for non-import items.

`calculate_addresses()` assigns `LocalIdx` to every local declaration. Parameters and locals share the same index space within a function (parameters occupy indices 0..N, then locals follow).

`and` and `or` compile to Wasm `if/else/end` blocks to implement short-circuit evaluation. They are handled specially in `Compiler::expr` before the standard left/right operand push pattern.

The import module name is hardcoded as `"js"`.

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
| 7          | `not`, `-` (unary prefix)          |
| 6          | `*`, `/`, `%`                      |
| 5          | `+`, `-`                           |
| 4          | `>`, `>=`, `<`, `<=`               |
| 3          | `==`, `!=`                         |
| 2          | `and`                              |
| 1          | `or`                               |

The parser uses Pratt parsing (`expression_precedence(min_precedence)`).

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

- **Snapshot tests** (`assert_snapshot!`): the expected output is inlined after `@`. When creating new tests, leave this empty. Never run `cargo insta accept` .
- `semantic/test.rs`: tests named `valid_*` expect `"<no error>"`. Tests named `error_*` expect a formatted error string with source underlines.
- `compiler/test.rs`: compiles to WAT (WebAssembly Text format) using `wasmprinter`. Check WAT output to verify correct instruction selection and local index assignment.
- `emit/test.rs`: actually executes the Wasm binary using `wasmtime`. The host provides `js.print_int`, `js.print_float`, and `js.print_bool`. The helper `run_main(body)` wraps a snippet in import declarations + `export fn main() { … }` automatically.

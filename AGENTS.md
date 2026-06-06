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

- `lexer::Lexer::new(src).next_token()` / `next_non_trivial()`
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
    Block       { statements: Vec<Stmt> },
    Declaration { name, ty: TypeRef, initializer: Expr },   // let x T = expr
    Assignment  { target: Expr, value: Expr },              // x = expr
    Return      { expr: Expr },
    ExprStmt    { expr: Expr },
}
```

**`if` and `while` are not yet implemented.** The tokens exist in the lexer but the parser does not handle them and the AST has no corresponding `StmtKind` variants.

### Expressions

```rust
enum ExprKind {
    Literal  { kind: LiteralKind },                         // 1, 2.5, true
    Variable { name: Identifier },
    Unary    { op: UnOp, expr: Box<Expr> },                 // -x, not x
    Binary   { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Call     { callee: Box<Expr>, args: Vec<Expr> },
}
```

Call expressions only work with a `Variable` callee at the compiler stage (function pointers are not supported yet). The semantic pass enforces the callee is a function type.

### Types

```rust
enum TypeKind { Int, Float, Bool }              // in AST (syntax)
enum Type { Unit, Int, Float, Bool, Function { params, ret } }  // in semantic (runtime)
```

`Unit` is the type of functions with no return type annotation. It has no literal and cannot be stored in a variable.

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
8. **Tests**: add snapshot tests at each layer (`parser/test.rs`, `semantic/test.rs`, `compiler/test.rs`) and an end-to-end execution test in `emit/test.rs`.
9. **Documentation**: update the `language_tour.md` file and `AGENTS.md`

Each layer should be tested in isolation before moving to the next. The snapshot tests make regressions easy to catch.

---

## Test Conventions

- **Snapshot tests** (`assert_snapshot!`): the expected output is inlined after `@`. When creating new tests, leave this empty. Never run `cargo insta accept` .
- `semantic/test.rs`: tests named `valid_*` expect `"<no error>"`. Tests named `error_*` expect a formatted error string with source underlines.
- `compiler/test.rs`: compiles to WAT (WebAssembly Text format) using `wasmprinter`. Check WAT output to verify correct instruction selection and local index assignment.
- `emit/test.rs`: actually executes the Wasm binary using `wasmtime`. The host provides `js.print_int`, `js.print_float`, and `js.print_bool`. The helper `run_main(body)` wraps a snippet in import declarations + `export fn main() { … }` automatically.

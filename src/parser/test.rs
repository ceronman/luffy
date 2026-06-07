use crate::ast::{BlockKind, ItemKind, StmtKind};
use crate::{parser, pretty};
use insta::assert_snapshot;

fn parse_module(src: &str) -> String {
    match parser::parse(src) {
        Ok(m) => pretty::ast::module(&m).unwrap(),
        Err(e) => pretty::annotate_error_single(src, &e),
    }
}

fn parse_expr(expr: &str) -> String {
    let module = format!("fn foo() {{\n{expr}\n}}");
    match parser::parse(&module) {
        Ok(m) => {
            let ItemKind::Function { body, .. } = &m.items[0].kind else {
                return pretty::ast::module(&m).unwrap();
            };
            let BlockKind::Braces { statements } = &body.kind else {
                return pretty::ast::module(&m).unwrap();
            };
            let StmtKind::ExprStmt { expr: e } = &statements[0].kind else {
                return pretty::ast::module(&m).unwrap();
            };
            pretty::ast::expr(e).unwrap()
        }
        Err(e) => pretty::annotate_error_single(&module, &e),
    }
}

fn parse_stmt(stmt_src: &str) -> String {
    let module = format!("fn foo() {{\n{stmt_src}\n}}");
    match parser::parse(&module) {
        Ok(m) => {
            let ItemKind::Function { body, .. } = &m.items[0].kind else {
                return pretty::ast::module(&m).unwrap();
            };
            let BlockKind::Braces { statements } = &body.kind else {
                return pretty::ast::module(&m).unwrap();
            };
            pretty::ast::stmt(&statements[0]).unwrap()
        }
        Err(e) => pretty::annotate_error_single(&module, &e),
    }
}

// ── Literal tests ─────────────────────────────────────────────────────────────

#[test]
fn integer_literal() {
    assert_snapshot!(parse_expr("42"), @"Int[42]");
}

#[test]
fn float_literal() {
    assert_snapshot!(parse_expr("3.14"), @"Float[3.14]");
}

#[test]
fn bool_literal_true() {
    assert_snapshot!(parse_expr("true"), @"Bool[true]");
}

#[test]
fn bool_literal_false() {
    assert_snapshot!(parse_expr("false"), @"Bool[false]");
}

// ── Variable tests ────────────────────────────────────────────────────────────

#[test]
fn variable_reference() {
    assert_snapshot!(parse_expr("x"), @"Var [x]");
}

// ── Unary expression tests ────────────────────────────────────────────────────

#[test]
fn negation_of_integer() {
    assert_snapshot!(parse_expr("-5"), @r"
    Unary [-]
    └── Int[5]
    ");
}

#[test]
fn negation_of_variable() {
    assert_snapshot!(parse_expr("-x"), @r"
    Unary [-]
    └── Var [x]
    ");
}

// ── Binary expression tests ───────────────────────────────────────────────────

#[test]
fn addition() {
    assert_snapshot!(parse_expr("1 + 2"), @r"
    Binary [+]
    ├── Int[1]
    └── Int[2]
    ");
}

#[test]
fn subtraction() {
    assert_snapshot!(parse_expr("1 - 2"), @r"
    Binary [-]
    ├── Int[1]
    └── Int[2]
    ");
}

#[test]
fn multiplication() {
    assert_snapshot!(parse_expr("2 * 3"), @r"
    Binary [*]
    ├── Int[2]
    └── Int[3]
    ");
}

#[test]
fn division() {
    assert_snapshot!(parse_expr("10 / 2"), @r"
    Binary [/]
    ├── Int[10]
    └── Int[2]
    ");
}

#[test]
fn modulo() {
    assert_snapshot!(parse_expr("10 % 3"), @r"
    Binary [%]
    ├── Int[10]
    └── Int[3]
    ");
}

#[test]
fn equality() {
    assert_snapshot!(parse_expr("a == b"), @r"
    Binary [==]
    ├── Var [a]
    └── Var [b]
    ");
}

#[test]
fn inequality() {
    assert_snapshot!(parse_expr("a != b"), @r"
    Binary [!=]
    ├── Var [a]
    └── Var [b]
    ");
}

#[test]
fn less_than() {
    assert_snapshot!(parse_expr("a < b"), @r"
    Binary [<]
    ├── Var [a]
    └── Var [b]
    ");
}

#[test]
fn less_than_or_equal() {
    assert_snapshot!(parse_expr("a <= b"), @r"
    Binary [<=]
    ├── Var [a]
    └── Var [b]
    ");
}

#[test]
fn greater_than() {
    assert_snapshot!(parse_expr("a > b"), @r"
    Binary [>]
    ├── Var [a]
    └── Var [b]
    ");
}

#[test]
fn greater_than_or_equal() {
    assert_snapshot!(parse_expr("a >= b"), @r"
    Binary [>=]
    ├── Var [a]
    └── Var [b]
    ");
}

// ── Operator-precedence tests ─────────────────────────────────────────────────

#[test]
fn precedence_mul_over_add() {
    // `*` binds tighter than `+`, so `2 * 3` is the right operand of `+`
    assert_snapshot!(parse_expr("1 + 2 * 3"), @r"
    Binary [+]
    ├── Int[1]
    └── Binary [*]
        ├── Int[2]
        └── Int[3]
    ");
}

#[test]
fn precedence_add_is_left_associative() {
    // `1 + 2` is evaluated first, then `+ 3`
    assert_snapshot!(parse_expr("1 + 2 + 3"), @r"
    Binary [+]
    ├── Binary [+]
    │   ├── Int[1]
    │   └── Int[2]
    └── Int[3]
    ");
}

#[test]
fn precedence_arithmetic_over_comparison() {
    // `1 + 2` is computed before the `==` comparison
    assert_snapshot!(parse_expr("1 + 2 == 3"), @r"
    Binary [==]
    ├── Binary [+]
    │   ├── Int[1]
    │   └── Int[2]
    └── Int[3]
    ");
}

#[test]
fn precedence_comparison_over_equality() {
    // `<` and `>` bind tighter than `==`
    assert_snapshot!(parse_expr("a < b == c > d"), @r"
    Binary [==]
    ├── Binary [<]
    │   ├── Var [a]
    │   └── Var [b]
    └── Binary [>]
        ├── Var [c]
        └── Var [d]
    ");
}

// ── Grouping tests ────────────────────────────────────────────────────────────

#[test]
fn grouping_overrides_precedence() {
    assert_snapshot!(parse_expr("(1 + 2) * 3"), @r"
    Binary [*]
    ├── Binary [+]
    │   ├── Int[1]
    │   └── Int[2]
    └── Int[3]
    ");
}

#[test]
fn nested_grouping() {
    // Double parentheses around a simple literal — still just the literal
    assert_snapshot!(parse_expr("((42))"), @"Int[42]");
}

// ── Function-call tests ───────────────────────────────────────────────────────

#[test]
fn call_no_args() {
    assert_snapshot!(parse_expr("foo()"), @r"
    Call
    ├── Var [foo]
    └── Args
    ");
}

#[test]
fn call_single_arg() {
    assert_snapshot!(parse_expr("foo(1)"), @r"
    Call
    ├── Var [foo]
    └── Args
        └── Int[1]
    ");
}

#[test]
fn call_multiple_args() {
    assert_snapshot!(parse_expr("foo(1, 2, 3)"), @r"
    Call
    ├── Var [foo]
    └── Args
        ├── Int[1]
        ├── Int[2]
        └── Int[3]
    ");
}

#[test]
fn call_with_expression_arg() {
    assert_snapshot!(parse_expr("foo(1 + 2)"), @r"
    Call
    ├── Var [foo]
    └── Args
        └── Binary [+]
            ├── Int[1]
            └── Int[2]
    ");
}

// ── Statement tests ───────────────────────────────────────────────────────────

#[test]
fn declaration_int() {
    assert_snapshot!(parse_stmt("let x Int = 42"), @r"
    VarDeclaration [x]
    ├── Type
    │   └── Int
    └── Int[42]
    ");
}

#[test]
fn declaration_float() {
    assert_snapshot!(parse_stmt("let pi Float = 3.14"), @r"
    VarDeclaration [pi]
    ├── Type
    │   └── Float
    └── Float[3.14]
    ");
}

#[test]
fn declaration_bool() {
    assert_snapshot!(parse_stmt("let flag Bool = true"), @r"
    VarDeclaration [flag]
    ├── Type
    │   └── Bool
    └── Bool[true]
    ");
}

#[test]
fn assignment() {
    assert_snapshot!(parse_stmt("x = 42"), @r"
    Assign
    ├── Var [x]
    └── Int[42]
    ");
}

#[test]
fn return_integer() {
    assert_snapshot!(parse_stmt("return 42"), @r"
    Return
    └── Int[42]
    ");
}

#[test]
fn return_expression() {
    assert_snapshot!(parse_stmt("return a + b"), @r"
    Return
    └── Binary [+]
        ├── Var [a]
        └── Var [b]
    ");
}

#[test]
fn block_empty() {
    assert_snapshot!(parse_stmt("{}"), @"Block");
}

#[test]
fn block_single_statement() {
    assert_snapshot!(parse_stmt("{ return 1 }"), @r"
    Block
    └── Return
        └── Int[1]
    ");
}

#[test]
fn block_multiple_statements() {
    let src = r#"{
        let x Int = 1
        let y Int = 2
        return x
    }"#;
    assert_snapshot!(parse_stmt(src), @r"
    Block
    ├── VarDeclaration [x]
    │   ├── Type
    │   │   └── Int
    │   └── Int[1]
    ├── VarDeclaration [y]
    │   ├── Type
    │   │   └── Int
    │   └── Int[2]
    └── Return
        └── Var [x]
    ");
}

// ── Function-declaration tests ────────────────────────────────────────────────

#[test]
fn function_no_params_no_return_type() {
    let src = "fn main() { return 0 }";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [main]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Return
                    └── Int[0]
    ");
}

#[test]
fn function_with_return_type() {
    // Return type is validated by the parser (no unknown-type error) even
    // though the pretty printer does not display it explicitly.
    let src = "fn answer() Int { return 42 }";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [answer]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Int
        └── Body
            └── Block
                └── Return
                    └── Int[42]
    ");
}

#[test]
fn function_single_param() {
    let src = "fn double(x Int) Int { return x }";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [double]
        ├── Export [false]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── x
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── Block
                └── Return
                    └── Var [x]
    ");
}

#[test]
fn function_multiple_params() {
    let src = "fn add(a Int, b Int) Int { return a + b }";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [add]
        ├── Export [false]
        ├── Parameters
        │   ├── Param
        │   │   ├── Name
        │   │   │   └── a
        │   │   └── Type
        │   │       └── Int
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── Block
                └── Return
                    └── Binary [+]
                        ├── Var [a]
                        └── Var [b]
    ");
}

#[test]
fn multiple_functions_in_module() {
    let src = r#"
        fn one() { return 1 }
        fn two() { return 2 }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    ├── Function [one]
    │   ├── Export [false]
    │   ├── Parameters
    │   ├── Return
    │   │   └── Unit
    │   └── Body
    │       └── Block
    │           └── Return
    │               └── Int[1]
    └── Function [two]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Return
                    └── Int[2]
    ");
}

// ── Short colon function-body tests ───────────────────────────────────────────

#[test]
fn short_body_single_expression() {
    let src = "fn add(a Int, b Int) Int: a + b";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [add]
        ├── Export [false]
        ├── Parameters
        │   ├── Param
        │   │   ├── Name
        │   │   │   └── a
        │   │   └── Type
        │   │       └── Int
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── ExprBlock
                └── Binary [+]
                    ├── Var [a]
                    └── Var [b]
    ");
}

#[test]
fn short_body_literal() {
    let src = "fn answer() Int: 42";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [answer]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Int
        └── Body
            └── ExprBlock
                └── Int[42]
    ");
}

#[test]
fn short_body_call_expression() {
    let src = "fn twice(x Int) Int: double(x)";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [twice]
        ├── Export [false]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── x
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── ExprBlock
                └── Call
                    ├── Var [double]
                    └── Args
                        └── Var [x]
    ");
}

#[test]
fn short_body_exported() {
    let src = "export fn add(a Int, b Int) Int: a + b";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [add]
        ├── Export [true]
        ├── Parameters
        │   ├── Param
        │   │   ├── Name
        │   │   │   └── a
        │   │   └── Type
        │   │       └── Int
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── ExprBlock
                └── Binary [+]
                    ├── Var [a]
                    └── Var [b]
    ");
}

#[test]
fn short_body_no_return_type() {
    // No return type annotation (Unit), so the colon follows the `)` directly.
    let src = "fn greet(): print_int(1)";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [greet]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── ExprBlock
                └── Call
                    ├── Var [print_int]
                    └── Args
                        └── Int[1]
    ");
}

#[test]
fn error_function_body_must_be_block_or_colon() {
    // A bare statement is no longer a valid function body.
    assert_snapshot!(parse_module("fn f() Int x = 5"), @"
    fn f() Int x = 5
               ^ ─── Expected `{` or `:` for function body, found <Identifier> instead
    ");
}

#[test]
fn error_short_body_requires_expression() {
    // After `:` an expression is required, not a statement.
    assert_snapshot!(parse_module("fn f() Int: return 5"), @"
    fn f() Int: return 5
                ^^^^^^ ─── Unexpected `return`
    ");
}

// ── Export / import declaration tests ────────────────────────────────────────

#[test]
fn export_function() {
    let src = "export fn add(a Int, b Int) Int { return a + b }";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [add]
        ├── Export [true]
        ├── Parameters
        │   ├── Param
        │   │   ├── Name
        │   │   │   └── a
        │   │   └── Type
        │   │       └── Int
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── Block
                └── Return
                    └── Binary [+]
                        ├── Var [a]
                        └── Var [b]
    ");
}

#[test]
fn import_function_no_params_no_return_type() {
    assert_snapshot!(parse_module("import fn noop()"), @"
    Module
    └── Import [noop]
        ├── Parameters
        └── Return
            └── Unit
    ");
}

#[test]
fn import_function_no_params_with_return_type() {
    assert_snapshot!(parse_module("import fn answer() Int"), @"
    Module
    └── Import [answer]
        ├── Parameters
        └── Return
            └── Int
    ");
}

#[test]
fn import_function_with_params_and_return_type() {
    assert_snapshot!(parse_module("import fn add(a Int, b Int) Int"), @"
    Module
    └── Import [add]
        ├── Parameters
        │   ├── Param
        │   │   ├── Name
        │   │   │   └── a
        │   │   └── Type
        │   │       └── Int
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Int
        └── Return
            └── Int
    ");
}

#[test]
fn module_with_import_and_export() {
    let src = r#"
        import fn log(n Int)
        export fn double(x Int) Int { return x + x }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    ├── Import [log]
    │   ├── Parameters
    │   │   └── Param
    │   │       ├── Name
    │   │       │   └── n
    │   │       └── Type
    │   │           └── Int
    │   └── Return
    │       └── Unit
    └── Function [double]
        ├── Export [true]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── x
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── Block
                └── Return
                    └── Binary [+]
                        ├── Var [x]
                        └── Var [x]
    ");
}

// ── Error tests ───────────────────────────────────────────────────────────────

#[test]
fn error_missing_fn_keyword() {
    assert_snapshot!(parse_module("foo() {}"), @"
    foo() {}
    ^^^ ─── Expected item declaration, found <Identifier> instead
    ");
}

#[test]
fn error_missing_function_name() {
    assert_snapshot!(parse_module("fn () {}"), @"
    fn () {}
       ^ ─── Expected function name, found `(` instead
    ");
}

#[test]
fn error_unknown_type_in_param() {
    assert_snapshot!(parse_module("fn f(x Unknown) {}"), @"
    fn f(x Unknown) {}
           ^^^^^^^ ─── Unknown type
    ");
}

#[test]
fn error_unknown_type_as_return_type() {
    assert_snapshot!(parse_module("fn f() Unknown {}"), @"
    fn f() Unknown {}
           ^^^^^^^ ─── Unknown type
    ");
}

#[test]
fn error_unexpected_token_in_expression() {
    // `@` is not a valid token, so it produces an Error token kind
    assert_snapshot!(parse_expr("@"), @"
    @
    ^ ─── Unexpected <Error>
    ");
}

#[test]
fn error_unclosed_call_parenthesis() {
    assert_snapshot!(parse_module("fn foo() { foo(1, 2 }"), @"
    fn foo() { foo(1, 2 }
                        ^ ─── Expected `)`, got `}`
    ");
}

#[test]
fn error_unclosed_grouping_parenthesis() {
    assert_snapshot!(parse_module("fn foo() { foo(1, 2 }"), @"
    fn foo() { foo(1, 2 }
                        ^ ─── Expected `)`, got `}`
    ");
}

#[test]
fn error_missing_fn_after_import() {
    // `import` must be immediately followed by `fn`
    assert_snapshot!(parse_module("import foo()"), @"
    import foo()
           ^^^ ─── Expected `fn`, got <Identifier>
    ");
}

#[test]
fn error_missing_fn_after_export() {
    // `export` must be immediately followed by `fn`
    assert_snapshot!(parse_module("export bar() {}"), @"
    export bar() {}
           ^^^ ─── Expected `fn`, got <Identifier>
    ");
}

// ── Logical operator tests ────────────────────────────────────────────────────

#[test]
fn logical_and() {
    assert_snapshot!(parse_expr("a and b"), @r"
    Binary [and]
    ├── Var [a]
    └── Var [b]
    ");
}

#[test]
fn logical_or() {
    assert_snapshot!(parse_expr("a or b"), @r"
    Binary [or]
    ├── Var [a]
    └── Var [b]
    ");
}

#[test]
fn precedence_and_over_or() {
    // `and` (precedence 2) binds tighter than `or` (precedence 1),
    // so `a or b and c` parses as `a or (b and c)`.
    assert_snapshot!(parse_expr("a or b and c"), @r"
    Binary [or]
    ├── Var [a]
    └── Binary [and]
        ├── Var [b]
        └── Var [c]
    ");
}

#[test]
fn precedence_comparisons_over_logical() {
    // Comparison operators (precedence 3-4) bind tighter than `and` (2),
    // so `a == b and c != d` parses as `(a == b) and (c != d)`.
    assert_snapshot!(parse_expr("a == b and c != d"), @r"
    Binary [and]
    ├── Binary [==]
    │   ├── Var [a]
    │   └── Var [b]
    └── Binary [!=]
        ├── Var [c]
        └── Var [d]
    ");
}

#[test]
fn logical_and_is_left_associative() {
    assert_snapshot!(parse_expr("a and b and c"), @r"
    Binary [and]
    ├── Binary [and]
    │   ├── Var [a]
    │   └── Var [b]
    └── Var [c]
    ");
}

#[test]
fn not_operator() {
    assert_snapshot!(parse_expr("not a"), @r"
    Unary [not]
    └── Var [a]
    ");
}

#[test]
fn not_binds_tighter_than_and() {
    // `not` has prefix precedence 7, `and` has infix precedence 2, so
    // `not a and b` parses as `(not a) and b`, not `not (a and b)`.
    assert_snapshot!(parse_expr("not a and b"), @r"
    Binary [and]
    ├── Unary [not]
    │   └── Var [a]
    └── Var [b]
    ");
}

#[test]
fn not_binds_tighter_than_or() {
    // Same reasoning as `and`: `not a or b` is `(not a) or b`.
    assert_snapshot!(parse_expr("not a or b"), @r"
    Binary [or]
    ├── Unary [not]
    │   └── Var [a]
    └── Var [b]
    ");
}

#[test]
fn not_binds_tighter_than_comparison() {
    // `not` prefix precedence (7) is higher than `==` infix precedence (3),
    // so `not a == b` is `(not a) == b`.
    assert_snapshot!(parse_expr("not a == b"), @r"
    Binary [==]
    ├── Unary [not]
    │   └── Var [a]
    └── Var [b]
    ");
}

#[test]
fn not_grouping_overrides_precedence() {
    // Parentheses force `not` to apply to the whole sub-expression.
    assert_snapshot!(parse_expr("not (a and b)"), @r"
    Unary [not]
    └── Binary [and]
        ├── Var [a]
        └── Var [b]
    ");
}

#[test]
fn double_not() {
    // `not not a` parses as `not (not a)` — each `not` takes the
    // immediately following atom as its operand.
    assert_snapshot!(parse_expr("not not a"), @r"
    Unary [not]
    └── Unary [not]
        └── Var [a]
    ");
}

// --- Statements: newlines and semicolons as separators ------------------------

#[test]
fn statements_separated_by_newlines() {
    let src = r#"
        fn f() {
            let x Int = 1
            let y Int = 2
            return x
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                ├── VarDeclaration [x]
                │   ├── Type
                │   │   └── Int
                │   └── Int[1]
                ├── VarDeclaration [y]
                │   ├── Type
                │   │   └── Int
                │   └── Int[2]
                └── Return
                    └── Var [x]
    ");
}

#[test]
fn statements_separated_by_semicolons_on_one_line() {
    let src = r#"
        fn f() { let x Int = 1; let y Int = 2; return x }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                ├── VarDeclaration [x]
                │   ├── Type
                │   │   └── Int
                │   └── Int[1]
                ├── VarDeclaration [y]
                │   ├── Type
                │   │   └── Int
                │   └── Int[2]
                └── Return
                    └── Var [x]
    ");
}

#[test]
fn statements_mixing_newlines_and_semicolons() {
    let src = r#"
        fn f() {
            let x Int = 1; let y Int = 2
            return x
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                ├── VarDeclaration [x]
                │   ├── Type
                │   │   └── Int
                │   └── Int[1]
                ├── VarDeclaration [y]
                │   ├── Type
                │   │   └── Int
                │   └── Int[2]
                └── Return
                    └── Var [x]
    ");
}

#[test]
fn trailing_and_repeated_semicolons_are_ignored() {
    // Leading, repeated, and trailing semicolons produce no empty statements.
    let src = r#"
        fn f() {
            ;;
            let x Int = 1 ;;;
            return x ;;
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                ├── VarDeclaration [x]
                │   ├── Type
                │   │   └── Int
                │   └── Int[1]
                └── Return
                    └── Var [x]
    ");
}

#[test]
fn blank_lines_between_statements_are_ignored() {
    let src = r#"
        fn f() {


            let x Int = 1


            return x

        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                ├── VarDeclaration [x]
                │   ├── Type
                │   │   └── Int
                │   └── Int[1]
                └── Return
                    └── Var [x]
    ");
}

// --- Items: newlines and semicolons as separators -----------------------------

#[test]
fn items_separated_by_newlines() {
    let src = r#"
        fn a() { return 1 }
        fn b() { return 2 }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    ├── Function [a]
    │   ├── Export [false]
    │   ├── Parameters
    │   ├── Return
    │   │   └── Unit
    │   └── Body
    │       └── Block
    │           └── Return
    │               └── Int[1]
    └── Function [b]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Return
                    └── Int[2]
    ");
}

#[test]
fn items_without_separator_on_one_line() {
    // Items need no separator at all — they parse one after another.
    let src = r#"
        fn a() { return 1 } fn b() { return 2 }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    ├── Function [a]
    │   ├── Export [false]
    │   ├── Parameters
    │   ├── Return
    │   │   └── Unit
    │   └── Body
    │       └── Block
    │           └── Return
    │               └── Int[1]
    └── Function [b]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Return
                    └── Int[2]
    ");
}

#[test]
fn error_semicolon_between_items() {
    // `;` is not valid between items (it is not an item keyword).
    let src = r#"
        fn a() { return 1 }; fn b() { return 2 }
    "#;
    assert_snapshot!(parse_module(src), @"
    fn a() { return 1 }; fn b() { return 2 }
                       ^ ─── Expected item declaration, found `;` instead
    ");
}

#[test]
fn error_leading_semicolon_at_module_level() {
    // A stray `;` before the first item is a parse error, not an empty item.
    let src = r#"
        ; fn a() { return 1 }
    "#;
    assert_snapshot!(parse_module(src), @"
    ; fn a() { return 1 }
    ^ ─── Expected item declaration, found `;` instead
    ");
}

#[test]
fn blank_and_leading_lines_at_module_level_are_ignored() {
    let src = r#"



        import fn log(n Int)


        fn a() { return 1 }

    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    ├── Import [log]
    │   ├── Parameters
    │   │   └── Param
    │   │       ├── Name
    │   │       │   └── n
    │   │       └── Type
    │   │           └── Int
    │   └── Return
    │       └── Unit
    └── Function [a]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Return
                    └── Int[1]
    ");
}

// --- Newlines are insignificant inside delimited / incomplete constructs -------

#[test]
fn newlines_within_function_signature() {
    // Newlines may appear between `fn` and the name, inside the parameter list,
    // before the return type, and before the body `{`.
    let src = r#"
        fn
        add(
            a Int,
            b Int
        ) Int
        {
            return a + b
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [add]
        ├── Export [false]
        ├── Parameters
        │   ├── Param
        │   │   ├── Name
        │   │   │   └── a
        │   │   └── Type
        │   │       └── Int
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Int
        ├── Return
        │   └── Int
        └── Body
            └── Block
                └── Return
                    └── Binary [+]
                        ├── Var [a]
                        └── Var [b]
    ");
}

#[test]
fn newlines_within_call_arguments() {
    let src = r#"
        fn f() {
            foo(
                1,
                2,
                3
            )
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Call
                    ├── Var [foo]
                    └── Args
                        ├── Int[1]
                        ├── Int[2]
                        └── Int[3]
    ");
}

#[test]
fn newlines_within_grouping() {
    let src = r#"
        fn f() {
            (
                1 + 2
            )
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Binary [+]
                    ├── Int[1]
                    └── Int[2]
    ");
}

#[test]
fn colon_body_spanning_lines() {
    let src = r#"
        fn answer() Int:
            1 + 2
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [answer]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Int
        └── Body
            └── ExprBlock
                └── Binary [+]
                    ├── Int[1]
                    └── Int[2]
    ");
}

#[test]
fn trailing_operator_continues_across_newline() {
    // An expression that is obviously incomplete at the end of a line (trailing
    // binary operator) continues on the next line: this is a single `1 + 2`.
    let src = r#"
        fn f() {
            1 +
            2
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Binary [+]
                    ├── Int[1]
                    └── Int[2]
    ");
}

#[test]
fn leading_operator_continues_across_newline() {
    // Design choice: binary operators continue the expression even when placed
    // at the start of the next line, so this is a single comparison `a == b`.
    let src = r#"
        fn f() {
            a
            == b
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Binary [==]
                    ├── Var [a]
                    └── Var [b]
    ");
}

#[test]
fn minus_on_next_line_is_subtraction() {
    // The ambiguous case: a leading `-` continues as subtraction (`a - b`); it
    // does NOT start a new `-b` statement.
    let src = r#"
        fn f() {
            a
            - b
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Binary [-]
                    ├── Var [a]
                    └── Var [b]
    ");
}

// --- Disambiguation: `(` at the start of a line is not a call -----------------

#[test]
fn open_paren_on_next_line_starts_a_new_statement() {
    // `foo` and `(1 + 2)` are two separate statements, because the `(` begins a
    // new line. Compare with `open_paren_on_same_line_is_a_call`.
    let src = r#"
        fn f() {
            foo
            (1 + 2)
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                ├── Var [foo]
                └── Binary [+]
                    ├── Int[1]
                    └── Int[2]
    ");
}

#[test]
fn open_paren_on_same_line_is_a_call() {
    let src = r#"
        fn f() {
            foo(1 + 2)
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── Call
                    ├── Var [foo]
                    └── Args
                        └── Binary [+]
                            ├── Int[1]
                            └── Int[2]
    ");
}

// --- Missing separators are errors --------------------------------------------

#[test]
fn error_two_declarations_on_one_line_without_separator() {
    let src = r#"
        fn f() { let x Int = 1 let y Int = 2 }
    "#;
    assert_snapshot!(parse_module(src), @"
    fn f() { let x Int = 1 let y Int = 2 }
                           ^^^ ─── Expected newline or `;` between statements, found `let`
    ");
}

#[test]
fn error_two_expression_statements_without_separator() {
    let src = r#"
        fn f() { foo() bar() }
    "#;
    assert_snapshot!(parse_module(src), @"
    fn f() { foo() bar() }
                   ^^^ ─── Expected newline or `;` between statements, found <Identifier>
    ");
}

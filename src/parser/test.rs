use crate::ast::{ItemKind, StmtKind};
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
            let StmtKind::Block { statements } = &body.kind else {
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
            let StmtKind::Block { statements } = &body.kind else {
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
    // The wrapper adds a newline after the expression, so the parser
    // hits an Eol token rather than Eof when the closing paren is absent.
    assert_snapshot!(parse_expr("foo(1, 2"), @"
    foo(1, 2
            ^ ─── Expected `)`, got <End of line>
    ");
}

#[test]
fn error_unclosed_grouping_parenthesis() {
    assert_snapshot!(parse_expr("(1 + 2"), @"
    (1 + 2
          ^ ─── Expected `)`, got <End of line>
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

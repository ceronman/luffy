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

// New snapshots left empty: run `cargo insta test && cargo insta review`.

#[test]
fn hexadecimal_literal() {
    assert_snapshot!(parse_expr("0xFF"), @"Int[255]");
}

#[test]
fn hexadecimal_literal_lowercase() {
    assert_snapshot!(parse_expr("0xff"), @"Int[255]");
}

#[test]
fn hexadecimal_literal_uppercase_prefix() {
    assert_snapshot!(parse_expr("0X1a"), @"Int[26]");
}

#[test]
fn octal_literal() {
    assert_snapshot!(parse_expr("0o17"), @"Int[15]");
}

#[test]
fn binary_literal() {
    assert_snapshot!(parse_expr("0b101"), @"Int[5]");
}

#[test]
fn decimal_literal_with_underscores() {
    assert_snapshot!(parse_expr("1_000_000"), @"Int[1000000]");
}

#[test]
fn hexadecimal_literal_with_underscores() {
    assert_snapshot!(parse_expr("0xDE_AD_BE_EF"), @"Int[3735928559]");
}

#[test]
fn octal_literal_with_underscores() {
    assert_snapshot!(parse_expr("0o1_7"), @"Int[15]");
}

#[test]
fn binary_literal_with_underscores() {
    assert_snapshot!(parse_expr("0b1010_0101"), @"Int[165]");
}

#[test]
fn float_literal_with_underscores() {
    assert_snapshot!(parse_expr("1_000.000_5"), @"Float[1000.0005]");
}

#[test]
fn max_int_literal_hex() {
    assert_snapshot!(parse_expr("0x7FFF_FFFF_FFFF_FFFF"), @"Int[9223372036854775807]");
}

// New float snapshots left empty: run `cargo insta test && cargo insta review`.

#[test]
fn float_leading_dot() {
    assert_snapshot!(parse_expr(".5"), @"Float[0.5]");
}

#[test]
fn float_trailing_dot() {
    assert_snapshot!(parse_expr("10."), @"Float[10]");
}

#[test]
fn float_scientific_notation() {
    assert_snapshot!(parse_expr("1e10"), @"Float[10000000000]");
}

#[test]
fn float_scientific_uppercase() {
    assert_snapshot!(parse_expr("1E10"), @"Float[10000000000]");
}

#[test]
fn float_scientific_negative_exponent() {
    assert_snapshot!(parse_expr("1.5e-3"), @"Float[0.0015]");
}

#[test]
fn float_scientific_positive_exponent() {
    assert_snapshot!(parse_expr("2E+8"), @"Float[200000000]");
}

#[test]
fn float_leading_dot_with_exponent() {
    assert_snapshot!(parse_expr(".5e3"), @"Float[500]");
}

#[test]
fn float_scientific_with_underscores() {
    assert_snapshot!(parse_expr("1_000.000_5e1_0"), @"Float[10000005000000]");
}

// ── Literal error cases ───────────────────────────────────────────────────────

#[test]
fn invalid_octal_digit() {
    assert_snapshot!(parse_expr("0o18"), @"
    0o18
    ^^^^ ─── Invalid octal integer literal `0o18`: invalid digit `8`
    ");
}

#[test]
fn invalid_hexadecimal_digit() {
    assert_snapshot!(parse_expr("0xGG"), @"
    0xGG
    ^^^^ ─── Invalid hexadecimal integer literal `0xGG`: invalid digit `G`
    ");
}

#[test]
fn invalid_binary_digit() {
    assert_snapshot!(parse_expr("0b12"), @"
    0b12
    ^^^^ ─── Invalid binary integer literal `0b12`: invalid digit `2`
    ");
}

#[test]
fn missing_digits_after_base_prefix() {
    assert_snapshot!(parse_expr("0x"), @"
    0x
    ^^ ─── Invalid hexadecimal integer literal `0x`: missing digits
    ");
}

#[test]
fn leading_underscore_separator_is_rejected() {
    assert_snapshot!(parse_expr("0x_FF"), @"
    0x_FF
    ^^^^^ ─── Invalid hexadecimal integer literal `0x_FF`: `_` separators must appear between digits
    ");
}

#[test]
fn trailing_underscore_separator_is_rejected() {
    assert_snapshot!(parse_expr("1_000_"), @"
    1_000_
    ^^^^^^ ─── Invalid decimal integer literal `1_000_`: `_` separators must appear between digits
    ");
}

#[test]
fn consecutive_underscore_separators_are_rejected() {
    assert_snapshot!(parse_expr("1__000"), @"
    1__000
    ^^^^^^ ─── Invalid decimal integer literal `1__000`: consecutive `_` separators are not allowed
    ");
}

#[test]
fn integer_literal_overflow() {
    assert_snapshot!(parse_expr("0xFFFF_FFFF_FFFF_FFFF"), @"
    0xFFFF_FFFF_FFFF_FFFF
    ^^^^^^^^^^^^^^^^^^^^^ ─── Unable to parse integer value `0xFFFF_FFFF_FFFF_FFFF`: number too large to fit in target type
    ");
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
    assert_snapshot!(parse_stmt("let x Int = 42"), @"
    VarDeclaration [x]
    ├── Type
    │   └── Type [Int]
    └── Int[42]
    ");
}

#[test]
fn declaration_float() {
    assert_snapshot!(parse_stmt("let pi Float = 3.14"), @"
    VarDeclaration [pi]
    ├── Type
    │   └── Type [Float]
    └── Float[3.14]
    ");
}

#[test]
fn declaration_bool() {
    assert_snapshot!(parse_stmt("let flag Bool = true"), @"
    VarDeclaration [flag]
    ├── Type
    │   └── Type [Bool]
    └── Bool[true]
    ");
}

#[test]
fn declaration_with_unknown_type_name_parses() {
    // The parser no longer validates type names — any identifier is accepted
    // as a type and captured verbatim; resolution (and rejection) happens later
    // during semantic analysis.
    assert_snapshot!(parse_stmt("let x Foo = 1"), @"
    VarDeclaration [x]
    ├── Type
    │   └── Type [Foo]
    └── Int[1]
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
fn assignment_is_right_associative() {
    // Assignment is an expression that binds looser than everything and is
    // right-associative, so `a = b = c` parses as `a = (b = c)`.
    assert_snapshot!(parse_stmt("a = b = c"), @"
    Assign
    ├── Var [a]
    └── Assign
        ├── Var [b]
        └── Var [c]
    ");
}

#[test]
fn assignment_as_while_colon_body() {
    // Assignment is now an expression of type Unit, so it can be the single
    // expression of a colon `while` body: `while c: a = a + 1`.
    assert_snapshot!(parse_stmt("while true: a = a + 1"), @"
    While
    ├── Condition
    │   └── Bool[true]
    └── Body
        └── ExprBlock
            └── Assign
                ├── Var [a]
                └── Binary [+]
                    ├── Var [a]
                    └── Int[1]
    ");
}

#[test]
fn assignment_continues_across_newline() {
    // Like a binary operator, `=` continues an expression across a newline, so
    // the multi-line and single-line forms parse to the same AST.
    let single = parse_stmt("a = a + 1");
    let multi = parse_stmt("a =\n            a + 1");
    assert_eq!(single, multi);
}

#[test]
fn grouped_assignment_parses_as_assignment() {
    // Grouping is transparent in the AST, but parentheses let an assignment
    // expression appear anywhere — including wrapped in its own group.
    assert_snapshot!(parse_stmt("(a = 1)"), @"
    Assign
    ├── Var [a]
    └── Int[1]
    ");
}

#[test]
fn parenthesized_assignment_target_overrides_associativity() {
    // Assignment is right-associative, so `a = b = c` groups as `a = (b = c)`.
    // Parentheses on the left force the other grouping: the target of the outer
    // assignment becomes the inner `a = b` assignment expression.
    assert_snapshot!(parse_stmt("(a = b) = c"), @"
    Assign
    ├── Assign
    │   ├── Var [a]
    │   └── Var [b]
    └── Var [c]
    ");
}

#[test]
fn assignment_value_grouping_changes_precedence() {
    // Parentheses in the value change how the right-hand side associates:
    // `a = (1 + 2) * 3` multiplies the sum, unlike `a = 1 + 2 * 3`.
    assert_snapshot!(parse_stmt("a = (1 + 2) * 3"), @"
    Assign
    ├── Var [a]
    └── Binary [*]
        ├── Binary [+]
        │   ├── Int[1]
        │   └── Int[2]
        └── Int[3]
    ");
}

#[test]
fn assignment_in_grouped_call_argument() {
    // Because assignment is an expression, it can be a call argument, with or
    // without an extra grouping around it.
    assert_snapshot!(parse_stmt("f(a = 1)"), @"
    Call
    ├── Var [f]
    └── Args
        └── Assign
            ├── Var [a]
            └── Int[1]
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
        │   └── Type [Int]
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
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
        │   │       └── Type [Int]
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
        │   │       └── Type [Int]
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
        │   └── Type [Int]
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
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
        │   │       └── Type [Int]
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
    assert_snapshot!(parse_module("fn f() Int: let x: Int = 5"), @"
    fn f() Int: let x: Int = 5
                ^^^ ─── Unexpected `let`
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
        │   │       └── Type [Int]
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
            └── Type [Int]
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
        │   │       └── Type [Int]
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Type [Int]
        └── Return
            └── Type [Int]
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
    │   │           └── Type [Int]
    │   └── Return
    │       └── Unit
    └── Function [double]
        ├── Export [true]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── x
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
                │   │   └── Type [Int]
                │   └── Int[1]
                ├── VarDeclaration [y]
                │   ├── Type
                │   │   └── Type [Int]
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
                │   │   └── Type [Int]
                │   └── Int[1]
                ├── VarDeclaration [y]
                │   ├── Type
                │   │   └── Type [Int]
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
                │   │   └── Type [Int]
                │   └── Int[1]
                ├── VarDeclaration [y]
                │   ├── Type
                │   │   └── Type [Int]
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
                │   │   └── Type [Int]
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
                │   │   └── Type [Int]
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
    │   │           └── Type [Int]
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
        │   │       └── Type [Int]
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
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
        │   └── Type [Int]
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

// ── `if` expression tests ─────────────────────────────────────────────────────

#[test]
fn if_expression_braces() {
    assert_snapshot!(parse_expr("if a > 0 { 1 } else { 2 }"), @"
    If
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    ├── Then
    │   └── Block
    │       └── Int[1]
    └── Else
        └── Block
            └── Int[2]
    ");
}

#[test]
fn if_expression_colon() {
    assert_snapshot!(parse_expr("if a > 0: 1 else: 2"), @"
    If
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    ├── Then
    │   └── ExprBlock
    │       └── Int[1]
    └── Else
        └── ExprBlock
            └── Int[2]
    ");
}

#[test]
fn if_statement_without_else() {
    assert_snapshot!(parse_expr("if a > 0 { print_int(1) }"), @"
    If
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    └── Then
        └── Block
            └── Call
                ├── Var [print_int]
                └── Args
                    └── Int[1]
    ");
}

#[test]
fn if_branch_with_multiple_statements() {
    let expr = r#"if a > 0 {
        print_int(1)
        2
    } else {
        3
    }"#;
    assert_snapshot!(parse_expr(expr), @"
    If
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    ├── Then
    │   └── Block
    │       ├── Call
    │       │   ├── Var [print_int]
    │       │   └── Args
    │       │       └── Int[1]
    │       └── Int[2]
    └── Else
        └── Block
            └── Int[3]
    ");
}

#[test]
fn nested_if_binds_else_to_nearest_if() {
    // The colon form: the `else` must bind to the inner `if`, not the outer one.
    assert_snapshot!(parse_expr("if a > 0: if b > 0: 1 else: 2"), @"
    If
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    └── Then
        └── ExprBlock
            └── If
                ├── Condition
                │   └── Binary [>]
                │       ├── Var [b]
                │       └── Int[0]
                ├── Then
                │   └── ExprBlock
                │       └── Int[1]
                └── Else
                    └── ExprBlock
                        └── Int[2]
    ");
}

// The braces form spanning multiple lines and the colon form on a single line
// must parse to the same AST. Both snapshots below are expected to be identical.

#[test]
fn if_braces_spanning_lines() {
    let src = r#"
        fn f(a Int) Int {
            if a > 0 {
                1
            } else {
                2
            }
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── a
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
        └── Body
            └── Block
                └── If
                    ├── Condition
                    │   └── Binary [>]
                    │       ├── Var [a]
                    │       └── Int[0]
                    ├── Then
                    │   └── Block
                    │       └── Int[1]
                    └── Else
                        └── Block
                            └── Int[2]
    ");
}

#[test]
fn if_else_keyword_on_new_line() {
    // `else` continues the `if` even when placed at the start of a new line.
    let src = r#"
        fn f(a Int) Int {
            if a > 0 {
                1
            }
            else {
                2
            }
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── a
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
        └── Body
            └── Block
                └── If
                    ├── Condition
                    │   └── Binary [>]
                    │       ├── Var [a]
                    │       └── Int[0]
                    ├── Then
                    │   └── Block
                    │       └── Int[1]
                    └── Else
                        └── Block
                            └── Int[2]
    ");
}

#[test]
fn else_if_chaining_without_colon() {
    let src = r#"
        fn f(a Int) Int {
            if a > 1 {
                1
            } else if a < 0 {
                2
            } else {
                3
            }
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── a
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Type [Int]
        └── Body
            └── Block
                └── If
                    ├── Condition
                    │   └── Binary [>]
                    │       ├── Var [a]
                    │       └── Int[1]
                    ├── Then
                    │   └── Block
                    │       └── Int[1]
                    └── Else
                        └── ExprBlock
                            └── If
                                ├── Condition
                                │   └── Binary [<]
                                │       ├── Var [a]
                                │       └── Int[0]
                                ├── Then
                                │   └── Block
                                │       └── Int[2]
                                └── Else
                                    └── Block
                                        └── Int[3]
    ");
}

#[test]
fn error_if_branch_missing_block_or_colon() {
    assert_snapshot!(parse_module("fn f(a Int) Int { return if a > 0 1 else 2 }"), @"
    fn f(a Int) Int { return if a > 0 1 else 2 }
                                      ^ ─── Expected `{` or `:` for `if` branch, found `<Int> instead
    ");
}

// ── `while` statement tests ───────────────────────────────────────────────────

#[test]
fn while_statement_braces() {
    assert_snapshot!(parse_stmt("while a > 0 { print_int(1) }"), @"
    While
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    └── Body
        └── Block
            └── Call
                ├── Var [print_int]
                └── Args
                    └── Int[1]
    ");
}

#[test]
fn while_statement_colon() {
    assert_snapshot!(parse_stmt("while a > 0: print_int(1)"), @"
    While
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    └── Body
        └── ExprBlock
            └── Call
                ├── Var [print_int]
                └── Args
                    └── Int[1]
    ");
}

#[test]
fn while_body_with_multiple_statements() {
    let src = r#"while a > 0 {
        print_int(a)
        a = a - 1
    }"#;
    assert_snapshot!(parse_stmt(src), @"
    While
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    └── Body
        └── Block
            ├── Call
            │   ├── Var [print_int]
            │   └── Args
            │       └── Var [a]
            └── Assign
                ├── Var [a]
                └── Binary [-]
                    ├── Var [a]
                    └── Int[1]
    ");
}

#[test]
fn while_braces_spanning_lines() {
    let src = r#"
        fn f(a Int) {
            while a > 0 {
                print_int(a)
                a = a - 1
            }
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── a
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── While
                    ├── Condition
                    │   └── Binary [>]
                    │       ├── Var [a]
                    │       └── Int[0]
                    └── Body
                        └── Block
                            ├── Call
                            │   ├── Var [print_int]
                            │   └── Args
                            │       └── Var [a]
                            └── Assign
                                ├── Var [a]
                                └── Binary [-]
                                    ├── Var [a]
                                    └── Int[1]
    ");
}

#[test]
fn while_single_line_with_semicolons() {
    let src = "fn f(a Int) { while a > 0 { print_int(a); a = a - 1 } }";
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [f]
        ├── Export [false]
        ├── Parameters
        │   └── Param
        │       ├── Name
        │       │   └── a
        │       └── Type
        │           └── Type [Int]
        ├── Return
        │   └── Unit
        └── Body
            └── Block
                └── While
                    ├── Condition
                    │   └── Binary [>]
                    │       ├── Var [a]
                    │       └── Int[0]
                    └── Body
                        └── Block
                            ├── Call
                            │   ├── Var [print_int]
                            │   └── Args
                            │       └── Var [a]
                            └── Assign
                                ├── Var [a]
                                └── Binary [-]
                                    ├── Var [a]
                                    └── Int[1]
    ");
}

#[test]
fn error_while_used_as_expression() {
    // `while` is a statement only — it never appears in expression position, so
    // using it as an initializer is a parse error.
    assert_snapshot!(parse_module("fn f() { let a Int = while true { 1 } }"), @"
    fn f() { let a Int = while true { 1 } }
                         ^^^^^ ─── Unexpected `while`
    ");
}

#[test]
fn error_while_body_missing_block_or_colon() {
    assert_snapshot!(parse_module("fn f() { while true print_int(1) }"), @"
    fn f() { while true print_int(1) }
                        ^^^^^^^^^ ─── Expected `{` or `:` for `while` body, found <Identifier> instead
    ");
}

// ── `break` / `continue` tests ────────────────────────────────────────────────

#[test]
fn break_parses_as_expression() {
    assert_snapshot!(parse_expr("break"), @"Break");
}

#[test]
fn continue_parses_as_expression() {
    assert_snapshot!(parse_expr("continue"), @"Continue");
}

#[test]
fn while_with_break_and_continue() {
    let src = r#"while a > 0 {
        if a > 5 { break }
        if a > 2 { continue }
        a = a - 1
    }"#;
    assert_snapshot!(parse_stmt(src), @"
    While
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    └── Body
        └── Block
            ├── If
            │   ├── Condition
            │   │   └── Binary [>]
            │   │       ├── Var [a]
            │   │       └── Int[5]
            │   └── Then
            │       └── Block
            │           └── Break
            ├── If
            │   ├── Condition
            │   │   └── Binary [>]
            │   │       ├── Var [a]
            │   │       └── Int[2]
            │   └── Then
            │       └── Block
            │           └── Continue
            └── Assign
                ├── Var [a]
                └── Binary [-]
                    ├── Var [a]
                    └── Int[1]
    ");
}

// ── `return` expression tests ─────────────────────────────────────────────────

#[test]
fn return_parses_as_expression() {
    assert_snapshot!(parse_expr("return 1 + 2"), @"
    Return
    └── Binary [+]
        ├── Int[1]
        └── Int[2]
    ");
}

#[test]
fn return_inside_if_branch() {
    assert_snapshot!(parse_stmt("if a > 0 { return 1 }"), @"
    If
    ├── Condition
    │   └── Binary [>]
    │       ├── Var [a]
    │       └── Int[0]
    └── Then
        └── Block
            └── Return
                └── Int[1]
    ");
}

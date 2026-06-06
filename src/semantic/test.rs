use crate::{parser, pretty, semantic};
use insta::assert_snapshot;

fn check(src: &str) -> String {
    match parser::parse(src) {
        Ok(module) => match semantic::semantic_analysis(&module) {
            Ok(_) => "<no error>".to_string(),
            Err(e) => pretty::annotate_error_single(src, &e),
        },
        Err(e) => format!("PARSE ERROR: {}", pretty::annotate_error_single(src, &e)),
    }
}

// ── Valid programs ────────────────────────────────────────────────────────────

#[test]
fn valid_integer_return() {
    assert_snapshot!(check("fn answer() Int { return 42 }"), @"<no error>");
}

#[test]
fn valid_float_return() {
    assert_snapshot!(check("fn pi() Float { return 3.14 }"), @"<no error>");
}

#[test]
fn valid_bool_return() {
    assert_snapshot!(check("fn yes() Bool { return true }"), @"<no error>");
}

#[test]
fn valid_unit_function_empty_body() {
    // A function with no return type annotation (Unit) and no return statement.
    assert_snapshot!(check("fn main() { }"), @"<no error>");
}

#[test]
fn valid_local_variable_declaration_and_use() {
    let src = r#"
        fn main() Int {
            let x Int = 1
            return x
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_variable_assignment() {
    let src = r#"
        fn main() {
            let x Int = 1
            x = 2
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_integer_arithmetic() {
    assert_snapshot!(check("fn add(a Int, b Int) Int { return a + b }"), @"<no error>");
}

#[test]
fn valid_float_arithmetic() {
    assert_snapshot!(check("fn add(a Float, b Float) Float { return a + b }"), @"<no error>");
}

#[test]
fn valid_integer_modulo() {
    assert_snapshot!(check("fn rem(a Int, b Int) Int { return a % b }"), @"<no error>");
}

#[test]
fn valid_comparison_produces_bool() {
    // `<` on numeric operands must yield Bool.
    assert_snapshot!(check("fn less(a Int, b Int) Bool { return a < b }"), @"<no error>");
}

#[test]
fn valid_equality_on_bools() {
    // `==` is valid for any matching types, including Bool.
    assert_snapshot!(check("fn eq(a Bool, b Bool) Bool { return a == b }"), @"<no error>");
}

#[test]
fn valid_function_call_with_correct_args() {
    let src = r#"
        fn double(x Int) Int { return x + x }
        fn main() Int { return double(21) }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_forward_function_reference() {
    // All functions are pre-declared before bodies are checked, so a function
    // may call one defined later in the file.
    let src = r#"
        fn main() Int { return helper() }
        fn helper() Int { return 42 }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

// ── Short colon function body ─────────────────────────────────────────────────

#[test]
fn valid_short_body_matches_return_type() {
    assert_snapshot!(check("fn add(a Int, b Int) Int: a + b"), @"<no error>");
}

#[test]
fn valid_short_body_bool() {
    assert_snapshot!(check("fn is_even(n Int) Bool: n % 2 == 0"), @"<no error>");
}

#[test]
fn valid_short_body_calling_other_function() {
    let src = r#"
        fn double(x Int) Int: x + x
        fn quad(x Int) Int: double(double(x))
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_short_body_no_return_type() {
    // No return type (Unit): the expression must also be Unit-typed, such as a
    // call to a Unit-returning function.
    let src = r#"
        import fn print_int(x Int)
        fn greet(): print_int(1)
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_short_body_type_mismatch() {
    // The expression's type must match the declared return type.
    assert_snapshot!(check("fn f() Int: true"), @"
    fn f() Int: true
                ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

// ── Name resolution errors ────────────────────────────────────────────────────

#[test]
fn error_undeclared_variable() {
    assert_snapshot!(check("fn main() Int { return x }"), @r"
    fn main() Int { return x }
                           ^ ─── Undeclared 'x'
    ");
}

#[test]
fn error_undeclared_function_call() {
    assert_snapshot!(check("fn main() Int { return unknown() }"), @r"
    fn main() Int { return unknown() }
                           ^^^^^^^ ─── Undeclared 'unknown'
    ");
}

#[test]
fn error_variable_used_before_its_declaration() {
    assert_snapshot!(check("fn main() { let x Int = x }"), @r"
    fn main() { let x Int = x }
                            ^ ─── Undeclared 'x'
    ");
}

#[test]
fn error_duplicate_variable_in_same_scope() {
    let src = r#"fn main() {
    let x Int = 1
    let x Int = 2
}"#;
    assert_snapshot!(check(src), @r"
    let x Int = 2
        ^ ─── Name 'x' is already declared in this scope
    ");
}

#[test]
fn error_duplicate_function_name() {
    let src = r#"
        fn foo() { }
        fn foo() { }
    "#;
    assert_snapshot!(check(src), @r"
    fn foo() { }
       ^^^ ─── Name 'foo' is already declared in this scope
    ");
}

// ── Type errors: return statements ───────────────────────────────────────────

#[test]
// TODO: Add special case error message for function that returns Unit
fn error_return_int_from_unit_function() {
    assert_snapshot!(check("fn main() { return 0 }"), @r"
    fn main() { return 0 }
                       ^ ─── Type mismatch: expected 'Unit', found 'Int'
    ");
}

#[test]
fn error_return_bool_from_int_function() {
    assert_snapshot!(check("fn main() Int { return true }"), @r"
    fn main() Int { return true }
                           ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_return_float_from_int_function() {
    assert_snapshot!(check("fn main() Int { return 1.0 }"), @r"
    fn main() Int { return 1.0 }
                           ^^^ ─── Type mismatch: expected 'Int', found 'Float'
    ");
}

// ── Type errors: variable declarations ───────────────────────────────────────

#[test]
fn error_declaration_initializer_type_mismatch() {
    assert_snapshot!(check("fn main() { let x Int = true }"), @r"
    fn main() { let x Int = true }
                            ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_declaration_float_initialised_with_int() {
    assert_snapshot!(check("fn main() { let x Float = 1 }"), @r"
    fn main() { let x Float = 1 }
                              ^ ─── Type mismatch: expected 'Float', found 'Int'
    ");
}

// ── Type errors: assignment ───────────────────────────────────────────────────

#[test]
fn error_assignment_type_mismatch() {
    let src = r#"fn main() {
        let x Int = 1
        x = true
    }"#;
    assert_snapshot!(check(src), @r"
    x = true
        ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

// ── Type errors: operators ────────────────────────────────────────────

#[test]
fn error_unary_operand_type_mismatch() {
    // Right operand has a different type from the left.
    assert_snapshot!(check("fn main() { -true }"), @"
    fn main() { -true }
                 ^^^^ ─── Operator requires numeric type
    ");
}

#[test]
fn error_binary_operand_type_mismatch() {
    // Right operand has a different type from the left.
    assert_snapshot!(check("fn main() { return 1 + true }"), @r"
    fn main() { return 1 + true }
                           ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_arithmetic_on_bool() {
    // Bool is not a numeric type; arithmetic operators are not defined for it.
    // TODO: improve error message
    assert_snapshot!(check("fn main() { return true + false }"), @r"
    fn main() { return true + false }
                       ^^^^ ─── Operator requires numeric type
    ");
}

#[test]
fn error_comparison_on_bool() {
    // Ordering comparisons require numeric operands.
    assert_snapshot!(check("fn main() Bool { return true < false }"), @r"
    fn main() Bool { return true < false }
                            ^^^^ ─── Operator requires numeric type
    ");
}

#[test]
fn error_modulo_on_float() {
    // `%` is only defined for integers; the error span covers the whole expression.
    assert_snapshot!(check("fn main() Float { return 1.0 % 2.0 }"), @r"
    fn main() Float { return 1.0 % 2.0 }
                             ^^^^^^^^^ ─── Modulo operator is not implemented for Float
    ");
}

// ── Type errors: function calls ───────────────────────────────────────────────

#[test]
// TODO: improve error message mentioning the actual type
fn error_call_on_non_function() {
    let src = r#"fn main() {
    let x Int = 1
    x()
}"#;
    assert_snapshot!(check(src), @r"
    x()
    ^ ─── Invalid function call: callee is not a function
    ");
}

#[test]
fn error_argument_type_mismatch() {
    let src = r#"fn add(a Int, b Int) Int { return a + b }
fn main() Int { return add(1, true) }"#;
    assert_snapshot!(check(src), @r"
    fn main() Int { return add(1, true) }
                                  ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_function_block_without_return() {
    let src = "fn add(a Int, b Int) Int { a + b }";
    assert_snapshot!(check(src), @r"
    fn add(a Int, b Int) Int { a + b }
                                     ^ ─── Missing return statement
    ");
}

#[test]
fn error_return_type_mismatch() {
    let src = r#"fn add(a Int, b Int) Int { return a + b }
fn main() Float { return add(1, 1) }"#;
    assert_snapshot!(check(src), @"
    fn main() Float { return add(1, 1) }
                             ^^^^^^^^^ ─── Type mismatch: expected 'Float', found 'Int'
    ");
}

// ── Export tests ──────────────────────────────────────────────────────────────

#[test]
fn valid_export_function() {
    // `export fn` is semantically identical to `fn`; the export flag is for
    // the code generator, not the type checker.
    assert_snapshot!(check("export fn add(a Int, b Int) Int { return a + b }"), @"<no error>");
}

#[test]
fn valid_export_function_callable_internally() {
    // An exported function can still be called by other functions in the module.
    let src = r#"
        export fn helper() Int { return 1 }
        fn main() Int { return helper() }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

// ── Import tests ──────────────────────────────────────────────────────────────

#[test]
fn valid_import_function_call() {
    // An imported function is pre-declared and can be called like any other.
    let src = r#"
        import fn add(a Int, b Int) Int
        fn main() Int { return add(1, 2) }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_import_duplicate_name() {
    let src = r#"
        import fn foo()
        import fn foo()
    "#;
    assert_snapshot!(check(src), @r"
    import fn foo()
              ^^^ ─── Name 'foo' is already declared in this scope
    ");
}

#[test]
fn error_import_and_function_name_clash() {
    // Declaring a function with the same name as an import is an error.
    let src = r#"
        import fn foo() Int
        fn foo() Int { return 1 }
    "#;
    assert_snapshot!(check(src), @r"
    fn foo() Int { return 1 }
       ^^^ ─── Name 'foo' is already declared in this scope
    ");
}

#[test]
fn error_call_imported_function_wrong_arg_type() {
    let src = r#"
        import fn add(a Int, b Int) Int
        fn main() Int { return add(1, true) }
    "#;
    assert_snapshot!(check(src), @r"
    fn main() Int { return add(1, true) }
                                  ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

// ── Logical operator tests ────────────────────────────────────────────────────

#[test]
fn valid_logical_and() {
    assert_snapshot!(check("fn f(a Bool, b Bool) Bool { return a and b }"), @"<no error>");
}

#[test]
fn valid_logical_or() {
    assert_snapshot!(check("fn f(a Bool, b Bool) Bool { return a or b }"), @"<no error>");
}

#[test]
fn valid_logical_combined_with_comparisons() {
    // Comparisons produce Bool, which is then fed into `and` / `or`.
    assert_snapshot!(check("fn f(a Int, b Int) Bool { return a < b and a != b }"), @"<no error>");
}

#[test]
fn error_logical_and_non_bool_type() {
    // Both operands are Int — same type, so check_ty_match passes, but then
    // the is_bool guard fires on the left operand's span.
    assert_snapshot!(check("fn main() { return 1 and 1 }"), @r"
    fn main() { return 1 and 1 }
                       ^ ─── Logical operator requires boolean type
    ");
}

#[test]
fn error_logical_or_non_bool_type() {
    assert_snapshot!(check("fn main() { return 1 or 1 }"), @r"
    fn main() { return 1 or 1 }
                       ^ ─── Logical operator requires boolean type
    ");
}

#[test]
fn error_logical_type_mismatch() {
    // The right operand is Int while the left is Bool — check_ty_match fires
    // on the right operand's span before the is_bool guard is even reached.
    assert_snapshot!(check("fn main() { return true and 1 }"), @"
    fn main() { return true and 1 }
                                ^ ─── Type mismatch: expected 'Bool', found 'Int'
    ");
}

// ── not operator tests ────────────────────────────────────────────────────────

#[test]
fn valid_not_operator() {
    assert_snapshot!(check("fn f(a Bool) Bool { return not a }"), @"<no error>");
}

#[test]
fn error_not_on_int() {
    // `not` requires a Bool operand; the error span points at the operand.
    assert_snapshot!(check("fn main() { return not 1 }"), @r"
    fn main() { return not 1 }
                           ^ ─── Operator requires boolean type
    ");
}

#[test]
fn error_not_on_float() {
    assert_snapshot!(check("fn main() { return not 1.5 }"), @r"
    fn main() { return not 1.5 }
                           ^^^ ─── Operator requires boolean type
    ");
}

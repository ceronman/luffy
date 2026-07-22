use crate::{parser, pretty, semantic};
use insta::assert_snapshot;

fn check(src: &str) -> String {
    match parser::parse(src) {
        Ok(module) => match semantic::semantic_analysis(&module) {
            Ok(_) => "<no error>".to_string(),
            Err(e) => pretty::annotate_error_single(src, &e),
        },
        Err(e) => pretty::annotate_error_single(src, &e),
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
fn valid_declaration_without_initializer() {
    // The initializer is optional. A `let` with only a type annotation declares
    // the binding and passes semantic analysis with no error.
    let src = r#"
        fn main() {
            let x Int
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_declaration_without_initializer_then_assigned() {
    // A binding declared without an initializer can be assigned afterwards and
    // then used, as long as the assigned value matches the declared type.
    let src = r#"
        fn main() Int {
            let x Int
            x = 7
            return x
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_assignment_to_uninitialized_declaration_type_mismatch() {
    // The declared type is still enforced for later assignments even when no
    // initializer was given.
    let src = r#"fn main() {
        let x Int
        x = true
    }"#;
    assert_snapshot!(check(src), @r"
    x = true
        ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn valid_assignment_in_while_colon_body() {
    // Assignment is an expression of type Unit, so it type-checks as the single
    // expression of a colon `while` body.
    let src = r#"
        fn main() {
            let a Int = 0
            while a < 3: a = a + 1
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_grouped_assignment() {
    // Grouping is transparent, so a parenthesized assignment is still a valid
    // `Unit` expression, and grouping in the value is fine too.
    let src = r#"
        fn main() {
            let a Int = 0
            (a = 1)
            a = (a + 1) * 2
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
fn error_unknown_type_in_param() {
    assert_snapshot!(check("fn f(x Unknown) {}"), @"
    fn f(x Unknown) {}
           ^^^^^^^ ─── Unknown type `Unknown`
    ");
}

#[test]
fn error_unknown_type_as_return_type() {
    assert_snapshot!(check("fn f() Unknown {}"), @"
    fn f() Unknown {}
           ^^^^^^^ ─── Unknown type `Unknown`
    ");
}

#[test]
fn error_unknown_type_in_declaration() {
    // The third site that resolves a `TypeRef` (after params and return types):
    // a `let` annotation.
    assert_snapshot!(check("fn f() { let x Foo = 1 }"), @"
    fn f() { let x Foo = 1 }
                   ^^^ ─── Unknown type `Foo`
    ");
}

#[test]
fn error_type_names_are_case_sensitive() {
    // Type resolution is an exact string match, so `int` is not `Int`.
    assert_snapshot!(check("fn f(x int) {}"), @"
    fn f(x int) {}
           ^^^ ─── Unknown type `int`
    ");
}

#[test]
fn valid_explicit_unit_return_type() {
    // `Unit` can now be written explicitly as a type name; it resolves the same
    // as omitting the return type.
    assert_snapshot!(check("fn f() Unit { }"), @"<no error>");
}

#[test]
fn error_return_int_from_unit_function() {
    assert_snapshot!(check("fn main() { return 0 }"), @"
    fn main() { return 0 }
                       ^ ─── Type mismatch: expected no value, found 'Int'
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
fn error_call_with_too_many_args() {
    let src = r#"
    fn foo(a Int, b Int) Int: 1
    fn main() {
        foo(1, 2, 3)
    }"#;
    assert_snapshot!(check(src), @"
    foo(1, 2, 3)
    ^^^ ─── Invalid function call: too many arguments
    ");
}

#[test]
fn error_call_with_not_enough_args() {
    let src = r#"
    fn foo(a Int, b Int) Int: 1
    fn main() {
        foo(1)
    }"#;
    assert_snapshot!(check(src), @"
    foo(1)
    ^^^ ─── Invalid function call: not enough arguments
    ");
}

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
    assert_snapshot!(check(src), @"
    fn add(a Int, b Int) Int { a + b }
                             ^^^^^^^^^ ─── Missing return statement
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

// ── `if` expression tests ─────────────────────────────────────────────────────

#[test]
fn valid_if_expression_braces() {
    assert_snapshot!(check("fn f(a Int) Int { return if a > 0 { 1 } else { 2 } }"), @"<no error>");
}

#[test]
fn valid_if_expression_colon() {
    assert_snapshot!(check("fn f(a Int) Int { return if a > 0: 1 else: 2 }"), @"<no error>");
}

#[test]
fn valid_if_expression_in_declaration() {
    let src = r#"
        fn f(a Int) Int {
            let b Int = if a > 0 { 1 } else { 2 }
            return b
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_if_statement_without_else() {
    let src = r#"
        import fn print_int(x Int)
        fn f(a Int) {
            if a > 0 {
                print_int(1)
            }
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_if_branch_last_expression_determines_type() {
    let src = r#"
        import fn print_int(x Int)
        fn f(a Int) Int {
            return if a > 0 {
                print_int(1)
                10
            } else {
                20
            }
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_if_expression_without_else() {
    assert_snapshot!(check("fn f(a Int) Int { let b Int = if a > 0: 0 }"), @"
    fn f(a Int) Int { let b Int = if a > 0: 0 }
                                  ^^^^^^^^^^^ ─── Type mismatch: expected 'Int', found 'Unit'
    ");
}

#[test]
fn error_if_branch_type_mismatch() {
    assert_snapshot!(check("fn f(a Int) Int { return if a > 0 { 1 } else { 2.5 } }"), @"
    fn f(a Int) Int { return if a > 0 { 1 } else { 2.5 } }
                                                 ^^^^^^^ ─── Type mismatch: expected 'Int', found 'Float'
    ");
}

#[test]
fn error_if_condition_not_bool() {
    assert_snapshot!(check("fn f(a Int) Int { return if a { 1 } else { 2 } }"), @"
    fn f(a Int) Int { return if a { 1 } else { 2 } }
                                ^ ─── Condition of 'if' must be 'Bool', found 'Int'
    ");
}

// ── `while` statement tests ───────────────────────────────────────────────────

#[test]
fn valid_while_braces() {
    assert_snapshot!(check("fn f(a Int) { while a > 0 { a = a - 1 } }"), @"<no error>");
}

#[test]
fn valid_while_colon() {
    let src = r#"
        import fn print_int(x Int)
        fn f(a Int) { while a > 0: print_int(a) }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_while_body_scopes_locals() {
    // A local declared in the loop body is scoped to the body.
    assert_snapshot!(check("fn f(a Int) { while a > 0 { let b Int = a; a = a - b } }"), @"<no error>");
}

#[test]
fn error_while_condition_not_bool() {
    assert_snapshot!(check("fn f(a Int) { while a { a = a - 1 } }"), @"
    fn f(a Int) { while a { a = a - 1 } }
                        ^ ─── Condition of 'while' must be 'Bool', found 'Int'
    ");
}

// ── `break` / `continue` tests ────────────────────────────────────────────────

#[test]
fn valid_break_in_while() {
    assert_snapshot!(check("fn f(a Int) { while a > 0 { break } }"), @"<no error>");
}

#[test]
fn valid_continue_in_while() {
    assert_snapshot!(check("fn f(a Int) { while a > 0 { a = a - 1; continue } }"), @"<no error>");
}

#[test]
fn valid_break_nested_in_if() {
    let src = r#"
        fn f(a Int) {
            while a > 0 {
                if a > 5 { break }
                a = a - 1
            }
        }"#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_break_outside_loop() {
    assert_snapshot!(check("fn f() { break }"), @"
    fn f() { break }
             ^^^^^ ─── 'break' outside of a loop
    ");
}

#[test]
fn error_continue_outside_loop() {
    assert_snapshot!(check("fn f() { continue }"), @"
    fn f() { continue }
             ^^^^^^^^ ─── 'continue' outside of a loop
    ");
}

// ── `return` / `Never` tests ──────────────────────────────────────────────────

#[test]
fn valid_tail_return() {
    assert_snapshot!(check("fn f() Int { return 1 }"), @"<no error>");
}

#[test]
fn valid_early_return_in_if() {
    // The block diverges via the trailing `return`, so it satisfies the Int
    // return type even though an `if` precedes it.
    let src = r#"
        fn f(a Int) Int {
            if a > 0 {
                return 1
            }
            return 2
        }"#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_if_else_both_return() {
    // Both branches diverge (`Never`), so the `if` statement has type `Never`
    // and counts as the function's diverging tail.
    let src = "fn f(a Int) Int { if a > 0 { return 1 } else { return 2 } }";
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_never_branch_takes_other_branch_type() {
    // `return` makes the then-branch `Never`; the `if` expression's type comes
    // from the else-branch (`Int`), so it can initialize an `Int`.
    let src = r#"
        fn f(a Int) Int {
            let x Int = if a > 0 {
                return 0
            } else { 5 }
            return x }
        "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_return_wrong_type() {
    assert_snapshot!(check("fn f() Int { return true }"), @"
    fn f() Int { return true }
                        ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_missing_return_non_unit() {
    assert_snapshot!(check("fn f() Int { let x Int = 1 }"), @"
    fn f() Int { let x Int = 1 }
               ^^^^^^^^^^^^^^^^^ ─── Missing return statement
    ");
}

// ── `Array` type tests ────────────────────────────────────────────────────────
//
// `Array` is the first generic built-in type. It requires exactly one
// argument: the element type, e.g. `Array[Int]`. Any other shape (no args or
// more than one) is rejected during type resolution.

#[test]
fn valid_array_type_in_param() {
    // The canonical, well-formed array type: a single element type.
    assert_snapshot!(check("fn f(x Array[Int]) {}"), @"<no error>");
}

#[test]
fn valid_array_of_floats() {
    // The element type can be any resolvable type, not just Int.
    assert_snapshot!(check("fn f(x Array[Float]) {}"), @"<no error>");
}

#[test]
fn valid_nested_array_type() {
    // The element type may itself be an array, since it is resolved recursively.
    assert_snapshot!(check("fn f(x Array[Array[Int]]) {}"), @"<no error>");
}

#[test]
fn error_array_no_arguments() {
    // `Array` with no arguments at all is not a complete type.
    assert_snapshot!(check("fn f(x Array) {}"), @"
    fn f(x Array) {}
           ^^^^^ ─── Array type requires one type argument, 0 given
    ");
}

#[test]
fn error_array_too_many_arguments() {
    // Exactly one argument is required; a second is invalid.
    assert_snapshot!(check("fn f(x Array[Int, Int]) {}"), @"
    fn f(x Array[Int, Int]) {}
           ^^^^^^^^^^^^^^^ ─── Array type requires one type argument, 2 given
    ");
}

#[test]
fn error_array_unknown_element_type() {
    // The shape is valid, but the element type is resolved and must exist, so an
    // unknown element type is reported at the inner type reference.
    assert_snapshot!(check("fn f(x Array[Foo]) {}"), @"
    fn f(x Array[Foo]) {}
                 ^^^ ─── Unknown type `Foo`
    ");
}

// ── Array indexing tests ──────────────────────────────────────────────────────
//
// Indexing `expr[index]` requires `expr` to be an `Array[T]` and `index` to be
// an `Int`. The result type is the element type `T`.

#[test]
fn valid_index_returns_element_type() {
    // Indexing an `Array[Int]` yields an `Int`, which matches the return type.
    assert_snapshot!(check("fn f(x Array[Int]) Int { return x[0] }"), @"<no error>");
}

#[test]
fn valid_index_element_type_float() {
    // The element type is propagated, so indexing an `Array[Float]` yields a Float.
    assert_snapshot!(check("fn f(x Array[Float]) Float { return x[0] }"), @"<no error>");
}

#[test]
fn valid_index_with_int_variable_index() {
    // The index may be any `Int` expression, not just a literal.
    assert_snapshot!(check("fn f(x Array[Int], i Int) Int { return x[i] }"), @"<no error>");
}

#[test]
fn valid_index_result_used_in_arithmetic() {
    // The element type flows into the surrounding expression.
    assert_snapshot!(check("fn f(x Array[Int]) Int { return x[0] + 1 }"), @"<no error>");
}

#[test]
fn valid_nested_index() {
    // Indexing an `Array[Array[Int]]` once yields `Array[Int]`; indexing again
    // yields `Int`.
    assert_snapshot!(check("fn f(x Array[Array[Int]]) Int { return x[0][1] }"), @"<no error>");
}

#[test]
fn error_index_non_array() {
    // Only arrays can be indexed; indexing a non-array value is rejected at the
    // indexed expression.
    assert_snapshot!(check("fn f(x Int) Int { return x[0] }"), @"
    fn f(x Int) Int { return x[0] }
                             ^ ─── Type mismatch: expected Array, found 'Int'
    ");
}

#[test]
fn error_index_non_int_index() {
    // The index must be an `Int`; a non-integer index is rejected at the index
    // expression.
    assert_snapshot!(check("fn f(x Array[Int]) Int { return x[1.5] }"), @"
    fn f(x Array[Int]) Int { return x[1.5] }
                                      ^^^ ─── Type mismatch: index expected as Int, found 'Float'
    ");
}

// ── Collection literal tests ──────────────────────────────────────────────────
//
// A collection literal `[a, b, c]` has type `Collection[T]`, where all elements
// must unify to a common element type `T`. An empty collection has no element
// type yet. A `Collection[T]` unifies with an `Array[T]`, so a collection
// literal can initialize an array-typed binding.

#[test]
fn valid_collection_assigned_to_array() {
    // A homogeneous collection unifies with the declared array type.
    assert_snapshot!(check("fn f() { let x Array[Int] = [1, 2, 3] }"), @"<no error>");
}

#[test]
fn valid_single_element_collection() {
    assert_snapshot!(check("fn f() { let x Array[Int] = [42] }"), @"<no error>");
}

#[test]
fn valid_empty_collection_assigned_to_array() {
    // An empty collection has no element type and unifies with any array type.
    assert_snapshot!(check("fn f() { let x Array[Int] = [] }"), @"<no error>");
}

#[test]
fn valid_collection_of_floats() {
    // The element type is inferred from the elements, here `Float`.
    assert_snapshot!(check("fn f() { let x Array[Float] = [1.0, 2.0] }"), @"<no error>");
}

#[test]
fn valid_nested_collection() {
    // Nested collections unify with nested array types element-wise.
    assert_snapshot!(check("fn f() { let x Array[Array[Int]] = [[1, 2], [3]] }"), @"<no error>");
}

#[test]
fn error_collection_heterogeneous_elements() {
    // All elements must unify to a single element type; mixing Int and Float is
    // rejected at the offending element.
    assert_snapshot!(check("fn f() { let x Array[Int] = [1, 2.0] }"), @"
    fn f() { let x Array[Int] = [1, 2.0] }
                                    ^^^ ─── Type mismatch: expected 'Int', found 'Float'
    ");
}

#[test]
fn error_collection_element_type_mismatch_with_array() {
    // The collection's element type must match the declared array element type.
    assert_snapshot!(check("fn f() { let x Array[Float] = [1, 2] }"), @"
    fn f() { let x Array[Float] = [1, 2] }
                                   ^ ─── Type mismatch: expected 'Float', found 'Int'
    ");
}

#[test]
fn error_collection_element_type_mismatch_with_if_blocks() {
    assert_snapshot!(check("fn f() Array[Int]: if true { [1, 2] } else { [1.0, 2.0] }"), @"
    fn f() Array[Int]: if true { [1, 2] } else { [1.0, 2.0] }
                                                  ^^^ ─── Type mismatch: expected 'Int', found 'Float'
    ");
}

#[test]
fn error_collection_element_type_mismatch_with_if_expr() {
    assert_snapshot!(check("fn f() Array[Int]: if true: [1, 2] else: [1.0, 2.0]"), @"
    fn f() Array[Int]: if true: [1, 2] else: [1.0, 2.0]
                                              ^^^ ─── Type mismatch: expected 'Int', found 'Float'
    ");
}

#[test]
fn error_collection_element_type_mismatch_with_call() {
    assert_snapshot!(check("fn f() Array[Int]: [true, false]"), @"
    fn f() Array[Int]: [true, false]
                        ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_collection_element_type_mismatch_with_call_and_return() {
    assert_snapshot!(check("fn f() Array[Int] { let a Int = 1; return [true, false] }"), @"
    fn f() Array[Int] { let a Int = 1; return [true, false] }
                                               ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

// ── Array index assignment (writing through an index) ─────────────────────────
//
// `arr[i] = v` is an assignment whose target is an `Index`. The index target
// resolves to the element type, and the assigned value must match it. The whole
// assignment has type `Unit`.

#[test]
fn valid_array_index_assignment() {
    assert_snapshot!(check("fn f(x Array[Int]) { x[0] = 1 }"), @"<no error>");
}

#[test]
fn valid_array_index_assignment_with_variable_index() {
    // The index may be any `Int` expression.
    assert_snapshot!(check("fn f(x Array[Int], i Int) { x[i] = i }"), @"<no error>");
}

#[test]
fn valid_nested_array_index_assignment() {
    // Writing through two levels of indexing: the target type is the innermost
    // element type (`Int`).
    assert_snapshot!(check("fn f(x Array[Array[Int]]) { x[0][1] = 5 }"), @"<no error>");
}

#[test]
fn valid_array_index_assignment_from_collection() {
    // The value written to an `Array[Array[Int]]` element is itself a collection
    // that unifies with `Array[Int]`.
    assert_snapshot!(check("fn f(x Array[Array[Int]]) { x[0] = [1, 2] }"), @"<no error>");
}

#[test]
fn error_array_index_assignment_value_type_mismatch() {
    // The written value must match the element type.
    let src = r#"fn f(x Array[Int]) {
        x[0] = true
    }"#;
    assert_snapshot!(check(src), @r"
    x[0] = true
           ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_array_index_assignment_on_non_array() {
    // Indexing a non-array on the left of an assignment is rejected when the
    // index target is resolved.
    let src = r#"fn f() {
        let x Int = 1
        x[0] = 5
    }"#;
    assert_snapshot!(check(src), @r"
    x[0] = 5
    ^ ─── Type mismatch: expected Array, found 'Int'
    ");
}

#[test]
fn error_array_index_assignment_non_int_index() {
    let src = r#"fn f(x Array[Int]) {
        x[true] = 1
    }"#;
    assert_snapshot!(check(src), @r"
    x[true] = 1
      ^^^^ ─── Type mismatch: index expected as Int, found 'Bool'
    ");
}

// ── Arrays as parameters / return values / call arguments ─────────────────────

#[test]
fn valid_array_returned_from_function() {
    assert_snapshot!(check("fn make() Array[Int]: [1, 2, 3]"), @"<no error>");
}

#[test]
fn valid_empty_array_returned_from_function() {
    assert_snapshot!(check("fn make() Array[Int]: []"), @"<no error>");
}

#[test]
fn valid_array_param_indexed_in_short_body() {
    assert_snapshot!(check("fn first(xs Array[Int]) Int: xs[0]"), @"<no error>");
}

#[test]
fn valid_collection_as_call_argument() {
    let src = r#"
        import fn sum(xs Array[Int]) Int
        fn f() Int { return sum([1, 2, 3]) }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_array_variable_as_call_argument() {
    let src = r#"
        import fn sum(xs Array[Int]) Int
        fn f(xs Array[Int]) Int { return sum(xs) }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

// ── Arrays of every element type ──────────────────────────────────────────────

#[test]
fn valid_array_of_bools() {
    assert_snapshot!(check("fn f() { let x Array[Bool] = [true, false] }"), @"<no error>");
}

#[test]
fn valid_index_of_bool_array_is_bool() {
    assert_snapshot!(check("fn f(x Array[Bool]) Bool: x[0]"), @"<no error>");
}

#[test]
fn valid_deeply_nested_array_index() {
    // Three levels of nesting resolve down to the innermost `Int`.
    assert_snapshot!(check("fn f(x Array[Array[Array[Int]]]) Int: x[0][1][2]"), @"<no error>");
}

#[test]
fn error_over_indexing_past_element_type() {
    // `x[0]` is already an `Int`; indexing it again is an error reported at the
    // inner `x[0]` operand.
    let src = r#"fn f(x Array[Int]) Int {
        return x[0][1]
    }"#;
    assert_snapshot!(check(src), @r"
    return x[0][1]
           ^^^^ ─── Type mismatch: expected Array, found 'Int'
    ");
}

#[test]
fn error_array_equality_not_supported() {
    // Equality used to type-check for arrays and then panic in codegen; it
    // is now rejected here.
    assert_snapshot!(check("fn f(a Array[Int], b Array[Int]) Bool: a == b"), @"
    fn f(a Array[Int], b Array[Int]) Bool: a == b
                                           ^^^^^^ ─── Equality operators are not supported for 'Array[Int]'
    ");
}

// ── Structs: valid programs ───────────────────────────────────────────────────

#[test]
fn valid_struct_declaration_only() {
    // A module containing only a struct declaration (no functions) type-checks.
    let src = r#"
        struct Point {
            x Int
            y Int
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_struct_construction_via_mapping() {
    // A mapping literal whose expected type is a struct constructs that struct.
    let src = r#"
        struct Point { x Int; y Int }
        fn make() Point: { x = 1, y = 2 }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_mapping_fields_out_of_order() {
    // Mapping fields are matched by name, so they may be written in any order.
    let src = r#"
        struct Point { x Int; y Int }
        fn make() Point: { y = 2, x = 1 }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_empty_struct_construction() {
    let src = r#"
        struct Empty {}
        fn make() Empty: {}
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_field_read() {
    let src = r#"
        struct Point { x Int; y Int }
        fn get_x(p Point) Int: p.x
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_field_assignment() {
    let src = r#"
        struct Point { x Int; y Int }
        fn set_x(p Point) { p.x = 5 }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_struct_passed_and_returned() {
    // A struct value can be a parameter and a return type; returning the
    // parameter unchanged type-checks (struct types unify by name).
    let src = r#"
        struct Point { x Int }
        fn id(p Point) Point: p
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_nested_struct_and_forward_reference() {
    // `Outer` refers to `Inner` before `Inner` is declared: all struct names are
    // registered before any field types are resolved, so forward references work.
    let src = r#"
        struct Outer { inner Inner; n Int }
        struct Inner { v Int }
        fn deep(o Outer) Int: o.inner.v
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_struct_with_array_field() {
    let src = r#"
        struct Bag { items Array[Int] }
        fn first(b Bag) Int: b.items[0]
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_struct_with_struct_field_construction() {
    // Nested construction: a mapping value can itself be a mapping for a struct
    // field, with the inner field type threaded as the expected type.
    let src = r#"
        struct Inner { v Int }
        struct Outer { inner Inner }
        fn make() Outer: { inner = { v = 1 } }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_mapping_field_value_infers_collection_type() {
    // The element type of an array field's collection literal is inferred from
    // the field's declared type.
    let src = r#"
        struct Bag { items Array[Int] }
        fn make() Bag: { items = [1, 2, 3] }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

// ── Structs: surprising / documented behaviors ────────────────────────────────

#[test]
fn surprising_duplicate_field_names_accepted_at_declaration() {
    // BUG/GAP: duplicate field names are NOT rejected when the struct is
    // declared. (Such a struct is then impossible to construct — see the
    // mapping tests — because the second field of the same name can never be
    // filled, but the declaration itself produces no error.)
    let src = r#"
        struct P {
            x Int
            x Float
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn surprising_recursive_struct_type_checks() {
    let src = r#"
        struct Node { value Int; next Node }
        fn first(n Node) Int: n.value
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn surprising_struct_named_after_builtin_is_accepted() {
    // TODO: Fix this
    // A struct may be named `Int`; the name resolves to the builtin everywhere a
    // type is expected, so the struct is effectively unusable as a type but its
    // declaration is accepted without error.
    let src = r#"
        struct Int { x Int }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

// ── Structs: error cases ──────────────────────────────────────────────────────

#[test]
fn error_field_access_on_non_struct() {
    assert_snapshot!(check("fn f(x Int) Int: x.foo"), @"
    fn f(x Int) Int: x.foo
                     ^ ─── Type mismatch: expected struct, found 'Int'
    ");
}

#[test]
fn error_field_not_in_struct() {
    assert_snapshot!(check("struct P { x Int } fn f(p P) Int: p.y"), @"
    struct P { x Int } fn f(p P) Int: p.y
                                        ^ ─── Type mismatch: field 'y' not found in struct 'P'
    ");
}

#[test]
fn error_mapping_missing_field() {
    assert_snapshot!(check("struct P { x Int; y Int } fn make() P: { x = 1 }"), @"
    struct P { x Int; y Int } fn make() P: { x = 1 }
                                           ^^^^^^^^^ ─── Type mismatch: mapping is missing field 'y'
    ");
}

#[test]
fn error_mapping_unknown_field() {
    assert_snapshot!(check("struct P { x Int } fn make() P: { x = 1, z = 2 }"), @"
    struct P { x Int } fn make() P: { x = 1, z = 2 }
                                             ^ ─── Field 'z' does not exist in type 'P'
    ");
}

#[test]
fn error_mapping_duplicate_field() {
    // A mapping literal that assigns the same field twice is rejected at the
    // second occurrence of the key.
    assert_snapshot!(check("struct P { x Int } fn make() P: { x = 1, x = 2 }"), @"
    struct P { x Int } fn make() P: { x = 1, x = 2 }
                                             ^ ─── Duplicate field 'x'
    ");
}

#[test]
fn error_mapping_wrong_field_type() {
    assert_snapshot!(check("struct P { x Int } fn make() P: { x = true }"), @"
    struct P { x Int } fn make() P: { x = true }
                                          ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_mapping_without_expected_type() {
    // A mapping with no expected type (here, an expression statement) cannot be
    // inferred. (The message says "collection" because mappings reuse the
    // collection inference path.)
    assert_snapshot!(check("fn f() { { a = 1 } }"), @"
    fn f() { { a = 1 } }
             ^^^^^^^^^ ─── Not enough information to infer type of mapping
    ");
}

#[test]
fn error_mapping_expected_non_struct_type() {
    assert_snapshot!(check("fn f() { let x Int = { a = 1 } }"), @"
    fn f() { let x Int = { a = 1 } }
                         ^^^^^^^^^ ─── Type mismatch: expected 'Int', found mapping
    ");
}

#[test]
fn error_field_assignment_wrong_type() {
    assert_snapshot!(check("struct P { x Int } fn f(p P) { p.x = true }"), @"
    struct P { x Int } fn f(p P) { p.x = true }
                                         ^^^^ ─── Type mismatch: expected 'Int', found 'Bool'
    ");
}

#[test]
fn error_unknown_struct_type_in_param() {
    assert_snapshot!(check("fn f(p Nonexistent) {}"), @"
    fn f(p Nonexistent) {}
           ^^^^^^^^^^^ ─── Unknown type `Nonexistent`
    ");
}

// ── Byte ─────────────────────────────────────────────────────────────────────

#[test]
fn valid_byte_declaration_from_literal() {
    // An integer literal types as `Byte` when the expected type is `Byte`.
    assert_snapshot!(check("fn f() { let b Byte = 65 }"), @"<no error>");
}

#[test]
fn valid_byte_literal_range_limits() {
    assert_snapshot!(check("fn f() { let lo Byte = 0; let hi Byte = 255 }"), @"<no error>");
}

#[test]
fn valid_byte_param_return_and_call_with_literal() {
    // Call arguments are an expected-type site, so a literal argument for a
    // `Byte` parameter infers `Byte`.
    let src = r#"
        fn id(b Byte) Byte { return b }
        fn f() Byte { return id(200) }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_byte_comparisons() {
    let src = r#"
        fn f(a Byte, b Byte) Bool {
            return a == b or a != b or a < b or a <= b or a > b or a >= b
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_byte_array_read_write() {
    let src = r#"
        fn f() Byte {
            let xs Array[Byte] = [72, 105]
            xs[0] = 255
            return xs[1]
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_byte_struct_field() {
    let src = "struct Pixel { r Byte; g Byte } fn f(p Pixel) Byte { return p.r }";
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_byte_literal_out_of_range() {
    assert_snapshot!(check("fn f() { let b Byte = 256 }"), @"
    fn f() { let b Byte = 256 }
                          ^^^ ─── Integer literal '256' is out of range for 'Byte' (0 to 255)
    ");
}

#[test]
fn error_byte_arithmetic() {
    // TODO: Byte has no arithmetic;
    assert_snapshot!(check("fn f(a Byte, b Byte) Byte { return a + b }"), @"
    fn f(a Byte, b Byte) Byte { return a + b }
                                       ^ ─── Operator requires numeric type
    ");
}

#[test]
fn error_byte_negation() {
    // TODO
    assert_snapshot!(check("fn f(b Byte) Byte { return -b }"), @"
    fn f(b Byte) Byte { return -b }
                                ^ ─── Operator requires numeric type
    ");
}

#[test]
fn error_byte_modulo() {
    // TODO
    assert_snapshot!(check("fn f(a Byte, b Byte) Byte { return a % b }"), @"
    fn f(a Byte, b Byte) Byte { return a % b }
                                       ^^^^^ ─── Modulo operator is not implemented for Byte
    ");
}

#[test]
fn error_int_variable_assigned_to_byte() {
    // Only literals infer Byte; an Int variable never implicitly converts.
    assert_snapshot!(check("fn f() { let x Int = 1; let b Byte = x }"), @"
    fn f() { let x Int = 1; let b Byte = x }
                                         ^ ─── Type mismatch: expected 'Byte', found 'Int'
    ");
}

#[test]
fn error_byte_compared_to_int_literal() {
    // Operands of a binary operator are checked without an expected type, so
    // the literal stays Int and does not unify with Byte.
    assert_snapshot!(check("fn f(b Byte) Bool { return b == 65 }"), @"
    fn f(b Byte) Bool { return b == 65 }
                                    ^^ ─── Type mismatch: expected 'Byte', found 'Int'
    ");
}

#[test]
fn error_negative_byte_literal() {
    // `-1` is unary negation of the literal `1`, which types as Int (negation
    // is not part of the literal), so it cannot initialize a Byte.
    assert_snapshot!(check("fn f() { let b Byte = -1 }"), @"
    fn f() { let b Byte = -1 }
                          ^^ ─── Type mismatch: expected 'Byte', found 'Int'
    ");
}

// ── Builtin functions ────────────────────────────────────────────────────────

#[test]
fn valid_int_builtins() {
    let src = r#"
        fn f() Int {
            return int_and(6, 3) + int_or(6, 3) + int_xor(6, 3) +
                int_shl(1, 4) + int_shr(-8, 1) + int_shr_u(-1, 60) +
                int_rotl(1, 3) + int_rotr(16, 3) + int_clz(1) + int_ctz(8) +
                int_popcnt(255) + int_div_u(7, 2) + int_rem_u(7, 2)
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_float_builtins() {
    let src = r#"
        fn f() Float {
            return float_min(float_sqrt(2.0), float_abs(-1.5)) +
                float_max(float_ceil(0.5), float_floor(1.5)) +
                float_copysign(float_trunc(2.7), float_nearest(-0.5))
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_conversion_builtins() {
    let src = r#"
        fn f() Byte {
            return int_to_byte(byte_to_int(int_to_byte(65)) + 1)
        }
        fn g() Int {
            return float_to_int(int_to_float(3))
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_byte_literal_comparison_via_conversion() {
    // int_to_byte gives the Byte-vs-literal comparison a workaround.
    let src = "fn f(b Byte) Bool { return b == int_to_byte(65) }";
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_builtin_redefinition() {
    // Builtins are pre-declared in the root scope, so a user function with
    // the same name collides.
    assert_snapshot!(check("fn int_and(a Int, b Int) Int: a"), @"
    fn int_and(a Int, b Int) Int: a
       ^^^^^^^ ─── Name 'int_and' is already declared in this scope
    ");
}

#[test]
fn error_builtin_wrong_arg_type() {
    assert_snapshot!(check("fn f() Int { return int_and(1.0, 2) }"), @"
    fn f() Int { return int_and(1.0, 2) }
                                ^^^ ─── Type mismatch: expected 'Int', found 'Float'
    ");
}

#[test]
fn error_builtin_wrong_arity() {
    assert_snapshot!(check("fn f() Float { return float_sqrt(1.0, 2.0) }"), @"
    fn f() Float { return float_sqrt(1.0, 2.0) }
                          ^^^^^^^^^^ ─── Invalid function call: too many arguments
    ");
}

// ── Strings ──────────────────────────────────────────────────────────────────

#[test]
fn valid_string_literal_declaration() {
    assert_snapshot!(check(r#"fn f() { let s String = "hello" }"#), @"<no error>");
}

#[test]
fn valid_empty_string_literal() {
    assert_snapshot!(check(r#"fn f() { let s String = "" }"#), @"<no error>");
}

#[test]
fn valid_string_param_and_return() {
    assert_snapshot!(check(r#"fn id(s String) String { return s }"#), @"<no error>");
}

#[test]
fn valid_string_in_struct_and_array() {
    let src = r#"
        struct Named { name String }
        fn f() String {
            let xs Array[String] = ["a", "b"]
            let n Named = { name = "Zoro" }
            return xs[0]
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_string_equality_not_supported() {
    assert_snapshot!(check("fn f(a String, b String) Bool: a == b"), @"
    fn f(a String, b String) Bool: a == b
                                   ^^^^^^ ─── Equality operators are not supported for 'String'
    ");
}

#[test]
fn error_struct_equality_not_supported() {
    assert_snapshot!(check("struct P { x Int } fn f(a P, b P) Bool: a == b"), @"
    struct P { x Int } fn f(a P, b P) Bool: a == b
                                            ^^^^^^ ─── Equality operators are not supported for 'P'
    ");
}

#[test]
fn error_string_concatenation_with_plus() {
    assert_snapshot!(check("fn f(a String, b String) String: a + b"), @"
    fn f(a String, b String) String: a + b
                                     ^ ─── Operator requires numeric type
    ");
}

#[test]
fn error_string_ordering_comparison() {
    assert_snapshot!(check("fn f(a String, b String) Bool: a < b"), @"
    fn f(a String, b String) Bool: a < b
                                   ^ ─── Operator requires numeric type
    ");
}

#[test]
fn error_string_indexing() {
    // Strings are not arrays: indexing is rejected, which also rules out
    // index assignment — the immutability story needs no extra checks.
    assert_snapshot!(check("fn f(s String) Byte: s[0]"), @"
    fn f(s String) Byte: s[0]
                         ^ ─── Type mismatch: expected Array, found 'String'
    ");
}

#[test]
fn error_string_index_assignment() {
    assert_snapshot!(check("fn f(s String) { s[0] = 65 }"), @"
    fn f(s String) { s[0] = 65 }
                     ^ ─── Type mismatch: expected Array, found 'String'
    ");
}

#[test]
fn error_int_assigned_to_string() {
    assert_snapshot!(check("fn f() { let s String = 1 }"), @"
    fn f() { let s String = 1 }
                            ^ ─── Type mismatch: expected 'String', found 'Int'
    ");
}

#[test]
fn valid_long_string_literal() {
    // Literals lower to data segments, so there is no length limit (the old
    // array.new_fixed lowering capped them at 10 000 bytes).
    let src = format!(r#"fn f() {{ let s String = "{}" }}"#, "a".repeat(100_000));
    assert_eq!(check(&src), "<no error>");
}

// ── String builtins ──────────────────────────────────────────────────────────

#[test]
fn valid_string_builtins() {
    let src = r#"
        fn f(s String, t String) String {
            let n Int = string_len(s)
            let b Byte = string_byte_at(s, 0)
            let eq Bool = string_eq(s, t)
            return string_concat(s, string_slice(t, 1, n))
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn valid_bytes_builtins() {
    let src = r#"
        fn f(s String) String {
            let bs Array[Byte] = string_to_bytes(s)
            let out Array[Byte] = bytes_new(string_len(s))
            bytes_copy(out, 0, bs, 0, string_len(s))
            return string_from_bytes(out)
        }
    "#;
    assert_snapshot!(check(src), @"<no error>");
}

#[test]
fn error_string_builtin_wrong_arg_type() {
    assert_snapshot!(check("fn f(s String) Int { return string_len(1) }"), @"
    fn f(s String) Int { return string_len(1) }
                                           ^ ─── Type mismatch: expected 'String', found 'Int'
    ");
}

#[test]
fn error_string_from_bytes_rejects_string() {
    // String and Array[Byte] share a runtime representation but stay
    // distinct in the type system.
    assert_snapshot!(check("fn f(s String) String { return string_from_bytes(s) }"), @"
    fn f(s String) String { return string_from_bytes(s) }
                                                     ^ ─── Type mismatch: expected 'Array[Byte]', found 'String'
    ");
}

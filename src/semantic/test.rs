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
           ^^^^^^^ ─── Unknown type
    ");
}

#[test]
fn error_unknown_type_as_return_type() {
    assert_snapshot!(check("fn f() Unknown {}"), @"
    fn f() Unknown {}
           ^^^^^^^ ─── Unknown type
    ");
}

#[test]
fn error_unknown_type_in_declaration() {
    // The third site that resolves a `TypeRef` (after params and return types):
    // a `let` annotation.
    assert_snapshot!(check("fn f() { let x Foo = 1 }"), @"
    fn f() { let x Foo = 1 }
                   ^^^ ─── Unknown type
    ");
}

#[test]
fn error_type_names_are_case_sensitive() {
    // Type resolution is an exact string match, so `int` is not `Int`.
    assert_snapshot!(check("fn f(x int) {}"), @"
    fn f(x int) {}
           ^^^ ─── Unknown type
    ");
}

#[test]
fn valid_explicit_unit_return_type() {
    // `Unit` can now be written explicitly as a type name; it resolves the same
    // as omitting the return type.
    assert_snapshot!(check("fn f() Unit { }"), @"<no error>");
}

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

// TODO: write a test for function calls with different arity.

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
                                  ^^^^^^^^^^^ ─── 'if' must have both main and 'else' branches when used as an expression.
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
                               ^ ─── Missing return statement
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
           ^^^^^ ─── Invalid array type
    ");
}

#[test]
fn error_array_too_many_arguments() {
    // Exactly one argument is required; a second is invalid.
    assert_snapshot!(check("fn f(x Array[Int, Int]) {}"), @"
    fn f(x Array[Int, Int]) {}
           ^^^^^^^^^^^^^^^ ─── Invalid array type
    ");
}

#[test]
fn error_array_unknown_element_type() {
    // The shape is valid, but the element type is resolved and must exist, so an
    // unknown element type is reported at the inner type reference.
    assert_snapshot!(check("fn f(x Array[Foo]) {}"), @"
    fn f(x Array[Foo]) {}
                 ^^^ ─── Unknown type
    ");
}

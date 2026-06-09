use crate::{compiler, emit, parser, pretty, semantic};
use insta::assert_snapshot;

pub fn run_to_stdout(binary: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_binary(&engine, binary)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let mut linker = wasmtime::Linker::new(&engine);
    let out = Arc::new(Mutex::new(Vec::new()));

    let out_int = Arc::clone(&out);
    linker.func_wrap(
        "js",
        "print_int",
        move |_caller: wasmtime::Caller<'_, ()>, i: i64| {
            writeln!(out_int.lock().unwrap(), "{i}").unwrap();
        },
    )?;

    let out_float = Arc::clone(&out);
    linker.func_wrap(
        "js",
        "print_float",
        move |_caller: wasmtime::Caller<'_, ()>, i: f64| {
            writeln!(out_float.lock().unwrap(), "{i}").unwrap();
        },
    )?;

    let out_bool = Arc::clone(&out);
    linker.func_wrap(
        "js",
        "print_bool",
        move |_caller: wasmtime::Caller<'_, ()>, i: i32| {
            let i = i != 0;
            writeln!(out_bool.lock().unwrap(), "{i}").unwrap();
        },
    )?;

    let instance = linker.instantiate(&mut store, &module)?;
    let main = instance.get_typed_func::<(), ()>(&mut store, "main")?;
    main.call(&mut store, ())?;

    let bytes = std::mem::take(&mut *out.lock().unwrap());
    Ok(bytes)
}

fn compile_and_run(src: &str) -> String {
    match parser::parse(&src) {
        Ok(module) => match semantic::semantic_analysis(&module) {
            Ok(semantics) => {
                let wasm_module = compiler::compile(&module, semantics);
                let binary = emit::emit(wasm_module);
                let out = run_to_stdout(&binary).unwrap();
                String::from_utf8(out).unwrap()
            }
            Err(e) => pretty::annotate_error_single(src, &e),
        },
        Err(e) => pretty::annotate_error_single(src, &e),
    }
}

fn run_main(body: &str) -> String {
    let mut src = String::new();
    src.push_str("import fn print_int(x Int)\n");
    src.push_str("import fn print_float(x Float)\n");
    src.push_str("import fn print_bool(x Bool)\n");
    src.push_str("export fn main() {\n");
    src.push_str(body);
    src.push_str("}\n");
    compile_and_run(&src)
}

#[test]
fn literals() {
    let src = r#"
        print_int(1)
        print_float(1.5)
        print_bool(true)
    "#;
    assert_snapshot!(run_main(src), @"
    1
    1.5
    true
    ");
}

#[test]
fn int_operators() {
    let src = r#"
        print_int(1 + 1)
        print_int(10 - 1)
        print_int(2 * 3)
        print_int(20 / 5)
        print_int(20 % 3)
        print_int(2 + 3 * 4)
        print_int((2 + 3) * 4)
    "#;
    assert_snapshot!(run_main(src), @"
    2
    9
    6
    4
    2
    14
    20
    ");
}

#[test]
fn not_operator() {
    // Covers basic behaviour and shows that `not` has higher prefix precedence
    // than `and`/`or` — e.g. `not true or false` is `(not true) or false`,
    // not `not (true or false)`.
    let src = r#"
        print_bool(not true)
        print_bool(not false)
        print_bool(not (1 == 2))
        print_bool(not (1 == 1))
        print_bool(not true or false)
        print_bool(not (true or false))
        print_bool(not true and not false)
    "#;
    assert_snapshot!(run_main(src), @"
    false
    true
    true
    false
    false
    false
    false
    ");
}

#[test]
fn float_operators() {
    // Whole-number float results (e.g. 3.0) are printed without a decimal point.
    let src = r#"
        print_float(1.0 + 1.5)
        print_float(3.5 - 1.5)
        print_float(1.5 * 2.0)
        print_float(7.0 / 2.0)
        print_float(1.0 + 2.0 * 1.5)
        print_float((1.0 + 2.0) * 1.5)
    "#;
    assert_snapshot!(run_main(src), @"
    2.5
    2
    3
    3.5
    4
    4.5
    ");
}

#[test]
fn negation() {
    // Integer negation compiles to `0 - x`; float negation uses `f64.neg`.
    let src = r#"
        print_int(-5)
        print_int(-100)
        print_float(-1.5)
        print_float(-2.5)
    "#;
    assert_snapshot!(run_main(src), @"
    -5
    -100
    -1.5
    -2.5
    ");
}

#[test]
fn int_comparisons() {
    // Each of the six comparison operators is exercised with both a true and a
    // false case, alternating to make the output easy to scan.
    let src = r#"
        print_bool(1 == 1)
        print_bool(1 == 2)
        print_bool(1 != 2)
        print_bool(2 != 2)
        print_bool(1 < 2)
        print_bool(2 <= 1)
        print_bool(2 > 1)
        print_bool(1 >= 2)
    "#;
    assert_snapshot!(run_main(src), @"
    true
    false
    true
    false
    true
    false
    true
    false
    ");
}

#[test]
fn float_comparisons() {
    let src = r#"
        print_bool(1.5 == 1.5)
        print_bool(1.5 == 2.5)
        print_bool(1.5 != 2.5)
        print_bool(1.5 < 2.5)
        print_bool(2.5 <= 1.5)
        print_bool(2.5 > 1.5)
        print_bool(1.5 >= 2.5)
    "#;
    assert_snapshot!(run_main(src), @"
    true
    false
    true
    true
    false
    true
    false
    ");
}

#[test]
fn variables() {
    // Covers: declaration of all three types, use in an expression, and
    // reassignment (which should overwrite the same local slot).
    let src = r#"
        let x Int = 10
        print_int(x)
        let y Float = 2.5
        print_float(y)
        let flag Bool = false
        print_bool(flag)
        let a Int = 3
        let b Int = 4
        print_int(a + b)
        x = 42
        print_int(x)
    "#;
    assert_snapshot!(run_main(src), @"
    10
    2.5
    false
    7
    42
    ");
}

#[test]
fn logical_operators() {
    // Full truth table for `and` and `or`.
    let src = r#"
        print_bool(true and true)
        print_bool(true and false)
        print_bool(false and true)
        print_bool(false and false)
        print_bool(false or false)
        print_bool(false or true)
        print_bool(true or false)
        print_bool(true or true)
    "#;
    assert_snapshot!(run_main(src), @"
    true
    false
    false
    false
    false
    true
    true
    true
    ");
}

#[test]
fn logical_short_circuit() {
    // When the result is determined by the left operand, the right operand
    // must NOT be evaluated.  `side_effect` prints its argument as a side
    // effect, so any call that should be skipped would leave a stray number
    // in the output.
    let src = r#"
        import fn print_int(x Int)
        import fn print_bool(x Bool)
        fn side_effect(n Int) Bool {
            print_int(n)
            return true
        }
        export fn main() {
            print_bool(false and side_effect(1))
            print_bool(true or side_effect(2))
            print_bool(true and side_effect(3))
            print_bool(false or side_effect(4))
        }
    "#;
    // 1 and 2 are NOT printed (short-circuited); 3 and 4 ARE printed.
    assert_snapshot!(compile_and_run(src), @"
    false
    true
    3
    true
    4
    true
    ");
}

#[test]
fn function_calls() {
    // compile_and_run is used here (instead of run_main) so that helper
    // functions can be defined alongside main.
    let src = r#"
        import fn print_int(x Int)
        fn add(a Int, b Int) Int { return a + b }
        fn square(x Int) Int { return x * x }
        export fn main() {
            print_int(add(3, 4))
            print_int(square(5))
            print_int(add(square(2), square(3)))
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    7
    25
    13
    ");
}

#[test]
fn expression_statements_drop_the_stack() {
    // compile_and_run is used here (instead of run_main) so that helper
    // functions can be defined alongside main.
    let src = r#"
        import fn print_int(x Int)
        fn square(x Int) Int { return x * x }
        export fn main() {
            print_int(256)
            square(2)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"256");
}

#[test]
fn short_body_functions() {
    // Functions defined with the short colon body run identically to ones
    // using a braces body with an explicit `return`.
    let src = r#"
        import fn print_int(x Int)
        import fn print_bool(x Bool)
        fn add(a Int, b Int) Int: a + b
        fn square(x Int) Int: x * x
        fn is_even(n Int) Bool: n % 2 == 0
        export fn main() {
            print_int(add(3, 4))
            print_int(square(5))
            print_bool(is_even(4))
            print_bool(is_even(7))
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    7
    25
    true
    false
    ");
}

// ── `if` expression execution ─────────────────────────────────────────────────

#[test]
fn if_expression_selects_then_branch() {
    let body = r#"
        let a Int = if 5 > 0 { 1 } else { 2 }
        print_int(a)
    "#;
    assert_snapshot!(run_main(body), @"1");
}

#[test]
fn if_expression_selects_else_branch() {
    let body = r#"
        let a Int = if 0 > 5: 1 else: 2
        print_int(a)
    "#;
    assert_snapshot!(run_main(body), @"2");
}

#[test]
fn if_statement_runs_then_branch_only_when_true() {
    let body = r#"
        if 5 > 0 {
            print_int(1)
        }
        if 0 > 5 {
            print_int(2)
        }
        print_int(3)
    "#;
    assert_snapshot!(run_main(body), @"
    1
    3
    ");
}

#[test]
fn if_branch_last_expression_is_the_value() {
    let body = r#"
        let a Int = if 5 > 0 {
            print_int(99)
            10
        } else {
            20
        }
        print_int(a)
    "#;
    assert_snapshot!(run_main(body), @"
    99
    10
    ");
}

// ── `while` statement execution ───────────────────────────────────────────────

#[test]
fn while_loop_counts_down() {
    let body = r#"
        let a Int = 3
        while a > 0 {
            print_int(a)
            a = a - 1
        }
        print_int(0)
    "#;
    assert_snapshot!(run_main(body), @"
    3
    2
    1
    0
    ");
}

#[test]
fn while_loop_body_never_runs_when_condition_false() {
    let body = r#"
        let a Int = 0
        while a > 0 {
            print_int(a)
            a = a - 1
        }
        print_int(99)
    "#;
    assert_snapshot!(run_main(body), @"99");
}

#[test]
fn while_loop_with_inner_local() {
    // A fresh local declared each iteration; sums 1+2+3 into `total`.
    let body = r#"
        let i Int = 1
        let total Int = 0
        while i <= 3 {
            let step Int = i
            total = total + step
            i = i + 1
        }
        print_int(total)
    "#;
    assert_snapshot!(run_main(body), @"6");
}

// ── `break` / `continue` execution ────────────────────────────────────────────

#[test]
fn break_exits_loop_early() {
    let body = r#"
        let i Int = 0
        while i < 10 {
            if i == 3 {
                break
            }
            print_int(i)
            i = i + 1
        }
        print_int(99)
    "#;
    assert_snapshot!(run_main(body), @"
    0
    1
    2
    99
    ");
}

#[test]
fn continue_skips_rest_of_body() {
    // `i` is incremented before the `continue`, so the loop still terminates;
    // the iteration where `i == 3` skips the `print_int`.
    let body = r#"
        let i Int = 0
        while i < 5 {
            i = i + 1
            if i == 3 {
                continue
            }
            print_int(i)
        }
    "#;
    assert_snapshot!(run_main(body), @"
    1
    2
    4
    5
    ");
}

// ── `return` execution ────────────────────────────────────────────────────────

#[test]
fn early_return_from_function() {
    let src = r#"
        import fn print_int(x Int)
        fn classify(n Int) Int {
            if n > 0 {
                return 1
            }
            return 0
        }
        export fn main() {
            print_int(classify(5))
            print_int(classify(-3))
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    1
    0
    ");
}

#[test]
fn return_value_from_if_else() {
    let src = r#"
        import fn print_int(x Int)
        fn pick(n Int) Int {
            let x Int = if n > 0 { return 0 } else { 5 }
            return x
        }
        export fn main() {
            print_int(pick(1))
            print_int(pick(-1))
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    0
    5
    ");
}

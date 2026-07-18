use crate::{compiler, emit, parser, pretty, semantic};
use insta::assert_snapshot;

pub fn run_to_stdout(binary: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    let mut config = wasmtime::Config::new();
    config.wasm_gc(true);
    config.wasm_function_references(true);
    let engine = wasmtime::Engine::new(&config)?;
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

    let out_string = Arc::clone(&out);
    linker.func_wrap(
        "js",
        "print_string",
        move |mut caller: wasmtime::Caller<'_, ()>,
              s: Option<wasmtime::Rooted<wasmtime::ArrayRef>>| {
            let bytes = emit::string_bytes(&mut caller, s);
            writeln!(
                out_string.lock().unwrap(),
                "{}",
                String::from_utf8_lossy(&bytes)
            )
            .unwrap();
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
    src.push_str("import fn print_string(x String)\n");
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
fn integer_literal_bases() {
    // Hexadecimal, octal and binary literals evaluate to the same values as
    // their decimal counterparts, and `_` separators are ignored.
    let src = r#"
        print_int(0xFF)
        print_int(0o17)
        print_int(0b101)
        print_int(0xDE_AD)
        print_int(1_000_000)
        print_int(0x7FFF_FFFF_FFFF_FFFF)
    "#;
    assert_snapshot!(run_main(src), @"
    255
    15
    5
    57005
    1000000
    9223372036854775807
    ");
}

#[test]
fn float_literal_forms() {
    // Leading-dot, trailing-dot and scientific-notation floats, plus `_`
    // separators, all evaluate as expected.
    let src = r#"
        print_float(.5)
        print_float(10.)
        print_float(1e3)
        print_float(1.5e-3)
        print_float(2E+8)
        print_float(.5e3)
        print_float(1_000.000_5)
    "#;
    assert_snapshot!(run_main(src), @"
    0.5
    10
    1000
    0.0015
    200000000
    500
    1000.0005
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
fn declaration_without_initializer_then_assigned() {
    // A binding declared without an initializer is assigned afterwards; the
    // assigned value is what gets printed.
    let src = r#"
        let x Int
        x = 5
        print_int(x)
        let y Float
        y = 2.5
        print_float(y)
    "#;
    assert_snapshot!(run_main(src), @"
    5
    2.5
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
fn imported_function_vs_defined_function_index() {
    let src = r#"
        fn foo(a Int) {
            print_int(1)
        }
        import fn print_int(a Int)
        export fn main() {
            foo(0)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"1");
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

#[test]
fn grouped_assignment_value() {
    // Parentheses in the assigned value change precedence: (1 + 2) * 3 = 9.
    let body = r#"
        let a Int = 0
        a = (1 + 2) * 3
        print_int(a)
    "#;
    assert_snapshot!(run_main(body), @"9");
}

#[test]
fn while_colon_body_with_grouped_assignment() {
    // A parenthesized assignment as a colon `while` body behaves like the
    // bare one: parentheses are transparent.
    let body = r#"
        let a Int = 0
        while a < 3: (a = a + 1)
        print_int(a)
    "#;
    assert_snapshot!(run_main(body), @"3");
}

#[test]
fn while_colon_body_with_assignment() {
    // Assignment is an expression of type Unit, so `while c: a = a + 1` runs the
    // assignment as the loop body and leaves nothing on the operand stack.
    let body = r#"
        let a Int = 0
        while a < 3: a = a + 1
        print_int(a)
    "#;
    assert_snapshot!(run_main(body), @"3");
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

// ── Array construction and indexing execution ─────────────────────────────────

#[test]
fn array_construct_and_index() {
    let body = r#"
        let a Array[Int] = [10, 20, 30]
        print_int(a[0])
        print_int(a[1])
        print_int(a[2])
    "#;
    assert_snapshot!(run_main(body), @"
    10
    20
    30
    ");
}

#[test]
fn array_index_with_expression_index() {
    // The index can be any Int expression, including a variable and arithmetic.
    let body = r#"
        let a Array[Int] = [5, 6, 7]
        let i Int = 1
        print_int(a[i])
        print_int(a[i + 1])
    "#;
    assert_snapshot!(run_main(body), @"
    6
    7
    ");
}

#[test]
fn array_write_through_index() {
    // Writing through an index mutates the array in place.
    let body = r#"
        let a Array[Int] = [1, 2, 3]
        a[1] = 99
        print_int(a[0])
        print_int(a[1])
        print_int(a[2])
    "#;
    assert_snapshot!(run_main(body), @"
    1
    99
    3
    ");
}

#[test]
fn array_of_floats() {
    let body = r#"
        let a Array[Float] = [1.5, 2.5]
        print_float(a[0])
        print_float(a[1])
    "#;
    assert_snapshot!(run_main(body), @"
    1.5
    2.5
    ");
}

#[test]
fn array_of_bools() {
    let body = r#"
        let a Array[Bool] = [true, false]
        print_bool(a[0])
        print_bool(a[1])
    "#;
    assert_snapshot!(run_main(body), @"
    true
    false
    ");
}

// ── Nested arrays ─────────────────────────────────────────────────────────────

#[test]
fn nested_array_read() {
    let body = r#"
        let a Array[Array[Int]] = [[1, 2], [3, 4], [5, 6]]
        print_int(a[0][0])
        print_int(a[1][1])
        print_int(a[2][0])
    "#;
    assert_snapshot!(run_main(body), @"
    1
    4
    5
    ");
}

#[test]
fn nested_array_write() {
    // Both writing a single inner element and replacing a whole inner array.
    let body = r#"
        let a Array[Array[Int]] = [[1, 2], [3, 4]]
        a[0][1] = 88
        a[1] = [7, 8]
        print_int(a[0][1])
        print_int(a[1][0])
        print_int(a[1][1])
    "#;
    assert_snapshot!(run_main(body), @"
    88
    7
    8
    ");
}

// ── Arrays through function boundaries ─────────────────────────────────────────

#[test]
fn array_as_parameter() {
    let src = r#"
        import fn print_int(x Int)
        fn first(xs Array[Int]) Int { return xs[0] }
        fn second(xs Array[Int]) Int: xs[1]
        export fn main() {
            let a Array[Int] = [11, 22, 33]
            print_int(first(a))
            print_int(second(a))
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    11
    22
    ");
}

#[test]
fn array_returned_from_function() {
    let src = r#"
        import fn print_int(x Int)
        fn make() Array[Int] { return [100, 200, 300] }
        export fn main() {
            let a Array[Int] = make()
            print_int(a[0])
            print_int(a[2])
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    100
    300
    ");
}

#[test]
fn collection_literal_as_call_argument() {
    let src = r#"
        import fn print_int(x Int)
        fn sum3(xs Array[Int]) Int { return xs[0] + xs[1] + xs[2] }
        export fn main() {
            print_int(sum3([4, 5, 6]))
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"15");
}

// ── Arrays combined with loops ────────────────────────────────────────────────

#[test]
fn array_filled_and_read_in_loops() {
    // Fill an array with i*i in one loop, then read it back in another.
    let body = r#"
        let a Array[Int] = [0, 0, 0, 0, 0]
        let i Int = 0
        while i < 5 {
            a[i] = i * i
            i = i + 1
        }
        i = 0
        while i < 5 {
            print_int(a[i])
            i = i + 1
        }
    "#;
    assert_snapshot!(run_main(body), @"
    0
    1
    4
    9
    16
    ");
}

#[test]
fn array_summed_in_loop() {
    let body = r#"
        let a Array[Int] = [1, 2, 3, 4]
        let total Int = 0
        let i Int = 0
        while i < 4 {
            total = total + a[i]
            i = i + 1
        }
        print_int(total)
    "#;
    assert_snapshot!(run_main(body), @"10");
}

// ── Struct construction, field read/write execution ───────────────────────────
//
// Structs are top-level items, so these tests use `compile_and_run` with a full
// program (imports + struct decls + `export fn main`) rather than `run_main`.

#[test]
fn struct_construct_and_read() {
    let src = r#"
        import fn print_int(x Int)
        struct Point { x Int; y Int }
        export fn main() {
            let p Point = { x = 10, y = 20 }
            print_int(p.x)
            print_int(p.y)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    10
    20
    ");
}

#[test]
fn struct_field_write() {
    let src = r#"
        import fn print_int(x Int)
        struct Point { x Int; y Int }
        export fn main() {
            let p Point = { x = 1, y = 2 }
            p.x = 99
            print_int(p.x)
            print_int(p.y)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    99
    2
    ");
}

#[test]
fn struct_construct_fields_out_of_order() {
    // The mapping lists fields in the opposite order from the declaration; the
    // compiler must emit the field values in declaration order so the struct is
    // built correctly.
    let src = r#"
        import fn print_int(x Int)
        struct Point { x Int; y Int }
        export fn main() {
            let p Point = { y = 20, x = 10 }
            print_int(p.x)
            print_int(p.y)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    10
    20
    ");
}

#[test]
fn struct_mixed_field_types() {
    let src = r#"
        import fn print_int(x Int)
        import fn print_float(x Float)
        import fn print_bool(x Bool)
        struct Mixed { a Int; b Float; c Bool }
        export fn main() {
            let m Mixed = { a = 1, b = 2.5, c = true }
            print_int(m.a)
            print_float(m.b)
            print_bool(m.c)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    1
    2.5
    true
    ");
}

#[test]
fn nested_struct_read_and_write() {
    let src = r#"
        import fn print_int(x Int)
        struct Inner { v Int }
        struct Outer { inner Inner; n Int }
        export fn main() {
            let o Outer = { inner = { v = 5 }, n = 9 }
            print_int(o.inner.v)
            print_int(o.n)
            o.inner.v = 42
            print_int(o.inner.v)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    5
    9
    42
    ");
}

#[test]
fn struct_with_array_field() {
    let src = r#"
        import fn print_int(x Int)
        struct Bag { items Array[Int] }
        export fn main() {
            let b Bag = { items = [7, 8, 9] }
            print_int(b.items[0])
            print_int(b.items[2])
            b.items[1] = 100
            print_int(b.items[1])
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    7
    9
    100
    ");
}

#[test]
fn struct_as_function_parameter() {
    let src = r#"
        import fn print_int(x Int)
        struct Point { x Int; y Int }
        fn sum(p Point) Int: p.x + p.y
        export fn main() {
            let p Point = { x = 3, y = 4 }
            print_int(sum(p))
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"7");
}

#[test]
fn struct_returned_from_function() {
    // A struct is constructed inside a function (colon body) and returned.
    let src = r#"
        import fn print_int(x Int)
        struct Point { x Int; y Int }
        fn origin() Point: { x = 0, y = 0 }
        export fn main() {
            let p Point = origin()
            print_int(p.x)
            print_int(p.y)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    0
    0
    ");
}

#[test]
fn struct_forward_reference_construction() {
    // `Outer` is declared before `Inner` it depends on.
    let src = r#"
        import fn print_int(x Int)
        struct Outer { inner Inner }
        struct Inner { v Int }
        export fn main() {
            let o Outer = { inner = { v = 123 } }
            print_int(o.inner.v)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"123");
}

#[test]
fn struct_has_reference_semantics() {
    // Struct values are GC references: passing a struct to a function and
    // mutating a field there is visible to the caller. (Worth being explicit
    // about — a reader might expect value/copy semantics.)
    let src = r#"
        import fn print_int(x Int)
        struct Box { v Int }
        fn bump(b Box) { b.v = b.v + 1 }
        export fn main() {
            let b Box = { v = 1 }
            bump(b)
            print_int(b.v)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"2");
}

#[test]
fn self_recursive_struct_builds_linked_list() {
    // A self-recursive struct works at runtime: an uninitialized local is a
    // null reference, so it can seed the tail of a list, and links can be
    // rewired to form arbitrary (even cyclic) shapes.
    let src = r#"
        import fn print_int(x Int)
        struct Node { data Int; next Node }
        export fn main() {
            let tail Node
            let head Node = { data = 1, next = tail }
            head.next = { data = 2, next = head }
            print_int(head.data)
            print_int(head.next.data)
            print_int(head.next.next.data)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    1
    2
    1
    ");
}

#[test]
fn mutually_recursive_structs_construct_and_read() {
    let src = r#"
        import fn print_int(x Int)
        struct A { value Int; b B }
        struct B { value Int; a A }
        export fn main() {
            let empty B
            let a A = { value = 1, b = { value = 2, a = { value = 3, b = empty } } }
            print_int(a.value)
            print_int(a.b.value)
            print_int(a.b.a.value)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    1
    2
    3
    ");
}

#[test]
fn empty_struct_constructs() {
    let src = r#"
        import fn print_int(x Int)
        struct Empty {}
        export fn main() {
            let e Empty = {}
            print_int(1)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"1");
}

// ── Byte ─────────────────────────────────────────────────────────────────────

#[test]
fn byte_comparisons() {
    // Byte has no printer yet; results are observed through print_bool.
    let src = r#"
        let a Byte = 65
        let b Byte = 200
        print_bool(a == a)
        print_bool(a == b)
        print_bool(a < b)
        print_bool(b <= a)
    "#;
    assert_snapshot!(run_main(src), @"
    true
    false
    true
    false
    ");
}

#[test]
fn byte_comparisons_are_unsigned() {
    // 200 stored in an i8 field would be negative if read sign-extended;
    // `array.get_u` + unsigned comparisons must see it as 200 > 100.
    let src = r#"
        let xs Array[Byte] = [200, 100]
        print_bool(xs[0] > xs[1])
    "#;
    assert_snapshot!(run_main(src), @"true");
}

#[test]
fn byte_array_read_write() {
    let src = r#"
        let xs Array[Byte] = [72, 105]
        let max Byte = 255
        print_bool(xs[0] < xs[1])
        xs[0] = max
        print_bool(xs[0] < xs[1])
        print_bool(xs[0] == max)
    "#;
    assert_snapshot!(run_main(src), @"
    true
    false
    true
    ");
}

#[test]
fn byte_through_functions_and_structs() {
    let src = r#"
        import fn print_bool(x Bool)
        struct Pixel { r Byte; g Byte }
        fn max_byte(a Byte, b Byte) Byte {
            if a < b { return b }
            return a
        }
        export fn main() {
            let p Pixel = { r = 255, g = 128 }
            print_bool(max_byte(p.r, p.g) == p.r)
            print_bool(p.g < p.r)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    true
    true
    ");
}

// ── Builtin functions ────────────────────────────────────────────────────────

/// Runs a main body that is expected to trap, returning the trap message.
fn run_main_trap(body: &str) -> String {
    let mut src = String::new();
    src.push_str("import fn print_int(x Int)\n");
    src.push_str("import fn print_float(x Float)\n");
    src.push_str("import fn print_bool(x Bool)\n");
    src.push_str("export fn main() {\n");
    src.push_str(body);
    src.push_str("}\n");
    let module = parser::parse(&src).expect("parse error");
    let semantics = semantic::semantic_analysis(&module).expect("semantic error");
    let wasm_module = compiler::compile(&module, semantics);
    let binary = emit::emit(wasm_module);
    match run_to_stdout(&binary) {
        Ok(_) => "<no trap>".to_string(),
        Err(e) => {
            // The trap is the root cause of the wasmtime error chain; the
            // outer layers carry an unstable wasm backtrace.
            let mut cause: &dyn std::error::Error = e.as_ref();
            while let Some(source) = cause.source() {
                cause = source;
            }
            cause.to_string()
        }
    }
}

#[test]
fn builtin_int_bitwise() {
    let src = r#"
        print_int(int_and(12, 10))
        print_int(int_or(12, 10))
        print_int(int_xor(12, 10))
    "#;
    assert_snapshot!(run_main(src), @"
    8
    14
    6
    ");
}

#[test]
fn builtin_int_shifts_and_rotates() {
    // int_shr is arithmetic (sign-preserving), int_shr_u is logical; the
    // rotate wraps bits around the i64 width.
    let src = r#"
        print_int(int_shl(1, 4))
        print_int(int_shr(-8, 1))
        print_int(int_shr_u(-1, 60))
        print_int(int_rotl(1, 3))
        print_int(int_rotr(16, 3))
        print_int(int_rotl(int_shl(1, 63), 1))
    "#;
    assert_snapshot!(run_main(src), @"
    16
    -4
    15
    8
    2
    1
    ");
}

#[test]
fn builtin_int_bit_counts() {
    let src = r#"
        print_int(int_clz(1))
        print_int(int_ctz(8))
        print_int(int_popcnt(255))
        print_int(int_clz(0))
    "#;
    assert_snapshot!(run_main(src), @"
    63
    3
    8
    64
    ");
}

#[test]
fn builtin_int_unsigned_div_rem() {
    // Negative operands are treated as huge unsigned values.
    let src = r#"
        print_int(int_div_u(7, 2))
        print_int(int_rem_u(7, 2))
        print_int(int_div_u(-2, 2))
        print_int(int_rem_u(-1, 10))
    "#;
    assert_snapshot!(run_main(src), @"
    3
    1
    9223372036854775807
    5
    ");
}

#[test]
fn builtin_float_rounding() {
    // float_nearest rounds half to even, per Wasm.
    let src = r#"
        print_float(float_ceil(2.3))
        print_float(float_floor(-2.3))
        print_float(float_trunc(-2.7))
        print_float(float_nearest(2.5))
        print_float(float_nearest(3.5))
    "#;
    assert_snapshot!(run_main(src), @"
    3
    -3
    -2
    2
    4
    ");
}

#[test]
fn builtin_float_math() {
    let src = r#"
        print_float(float_sqrt(9.0))
        print_float(float_abs(-1.5))
        print_float(float_min(1.5, 2.5))
        print_float(float_max(1.5, 2.5))
        print_float(float_copysign(3.0, -1.0))
    "#;
    assert_snapshot!(run_main(src), @"
    3
    1.5
    1.5
    2.5
    -3
    ");
}

#[test]
fn builtin_numeric_conversions() {
    // float_to_int truncates toward zero.
    let src = r#"
        print_float(int_to_float(3))
        print_int(float_to_int(3.9))
        print_int(float_to_int(-3.9))
    "#;
    assert_snapshot!(run_main(src), @"
    3
    3
    -3
    ");
}

#[test]
fn builtin_byte_conversions() {
    let src = r#"
        let b Byte = 200
        print_int(byte_to_int(b))
        print_bool(b == int_to_byte(200))
        let xs Array[Byte] = [65]
        print_int(byte_to_int(xs[0]) + 1)
        print_int(byte_to_int(int_to_byte(255)))
        print_int(byte_to_int(int_to_byte(0)))
    "#;
    assert_snapshot!(run_main(src), @"
    200
    true
    66
    255
    0
    ");
}

#[test]
fn builtin_int_to_byte_traps_above_range() {
    assert_snapshot!(run_main_trap("int_to_byte(256)"), @"wasm trap: wasm `unreachable` instruction executed");
}

#[test]
fn builtin_int_to_byte_traps_negative() {
    assert_snapshot!(run_main_trap("int_to_byte(-1)"), @"wasm trap: wasm `unreachable` instruction executed");
}

#[test]
fn builtin_float_to_int_traps_out_of_range() {
    assert_snapshot!(run_main_trap("float_to_int(1e300)"), @"wasm trap: integer overflow");
}

#[test]
fn builtin_int_div_u_traps_on_zero() {
    assert_snapshot!(run_main_trap("int_div_u(1, 0)"), @"wasm trap: integer divide by zero");
}

// ── Strings ──────────────────────────────────────────────────────────────────

#[test]
fn print_string_literal() {
    assert_snapshot!(run_main(r#"print_string("hello")"#), @"hello");
}

#[test]
fn print_string_escapes() {
    let src = r#"
        print_string("line1\nline2")
        print_string("say \"hi\"")
        print_string("a\tb")
    "#;
    assert_snapshot!(run_main(src), @r#"
    line1
    line2
    say "hi"
    a	b
    "#);
}

#[test]
fn print_string_unicode() {
    // Multibyte UTF-8 (2, 3 and 4 byte scalars) survives the byte-array
    // round trip, including \u{...} escapes.
    let src = r#"
        print_string("héllo €")
        print_string("\u{1F600}")
    "#;
    assert_snapshot!(run_main(src), @"
    héllo €
    😀
    ");
}

#[test]
fn print_empty_string() {
    let src = r#"
        print_string("a")
        print_string("")
        print_string("b")
    "#;
    assert_snapshot!(run_main(src), @"
    a

    b
    ");
}

#[test]
fn string_through_function_and_variable() {
    // A String flows through returns, locals and params; printing twice
    // shows the same reference is reusable.
    let src = r#"
        import fn print_string(s String)
        fn greet(name String) String { return "yo" }
        export fn main() {
            let s String = greet("Luffy")
            print_string(s)
            print_string(s)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    yo
    yo
    ");
}

#[test]
fn string_in_struct_field_and_array() {
    let src = r#"
        import fn print_string(s String)
        struct Named { name String }
        export fn main() {
            let xs Array[String] = ["a", "b"]
            print_string(xs[1])
            let n Named = { name = "Zoro" }
            print_string(n.name)
            n.name = "Nami"
            print_string(n.name)
        }
    "#;
    assert_snapshot!(compile_and_run(src), @"
    b
    Zoro
    Nami
    ");
}

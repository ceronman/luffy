use crate::{compiler, emit, parser, pretty, semantic};
use insta::assert_snapshot;

fn compile_to_wat(src: &str) -> String {
    match parser::parse(src) {
        Ok(module) => match semantic::semantic_analysis(&module) {
            Ok(semantics) => {
                let module = compiler::compile(&module, semantics);
                let wasm = emit::emit(module);
                wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary")
            }
            Err(e) => pretty::annotate_error_single(src, &e),
        },
        Err(e) => pretty::annotate_error_single(src, &e),
    }
}

// ── Literals and types ────────────────────────────────────────────────────────

#[test]
fn simple_function() {
    let wat = compile_to_wat(r#"fn main() Int { return 1 + 1 }"#);
    assert_snapshot!(wat, @"
    (module
      (type (;0;) (func (result i64)))
      (func (;0;) (type 0) (result i64)
        i64.const 1
        i64.const 1
        i64.add
        return
      )
    )
    ")
}

#[test]
fn short_body_compiles_like_braces() {
    // A short colon body leaves the expression value on the stack as the
    // function result, identical to a braces body with the same expression.
    let wat = compile_to_wat("fn add(a Int, b Int) Int: a + b");
    assert_snapshot!(wat, @"
    (module
      (type (;0;) (func (param i64 i64) (result i64)))
      (func (;0;) (type 0) (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.add
      )
    )
    ");
}

#[test]
fn return_integer_literal() {
    assert_snapshot!(compile_to_wat("fn main() Int { return 42 }"), @"
    (module
      (type (;0;) (func (result i64)))
      (func (;0;) (type 0) (result i64)
        i64.const 42
        return
      )
    )
    ");
}

#[test]
fn expression_statements_drop_the_stack() {
    let src = r#"
        import fn print_int(x Int)
        fn square(x Int) Int { return x * x }
        export fn main() {
            print_int(256)
            square(2)
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @r#"
    (module
      (type (;0;) (func (param i64)))
      (type (;1;) (func (param i64) (result i64)))
      (type (;2;) (func))
      (import "js" "print_int" (func (;0;) (type 0)))
      (export "main" (func 2))
      (func (;1;) (type 1) (param i64) (result i64)
        local.get 0
        local.get 0
        i64.mul
        return
      )
      (func (;2;) (type 2)
        i64.const 256
        call 0
        i64.const 2
        call 1
        drop
      )
    )
    "#);
}

#[test]
fn return_bool_literal() {
    // Bool maps to i32; `true` becomes i32.const 1, `false` becomes i32.const 0.
    assert_snapshot!(compile_to_wat("fn main() Bool { return true }"), @"
    (module
      (type (;0;) (func (result i32)))
      (func (;0;) (type 0) (result i32)
        i32.const 1
        return
      )
    )
    ");
}

#[test]
fn unit_function() {
    // A function with no return type has no result in the type signature.
    assert_snapshot!(compile_to_wat("fn main() { }"), @"
    (module
      (type (;0;) (func))
      (func (;0;) (type 0))
    )
    ");
}

// ── Integer arithmetic ────────────────────────────────────────────────────────

#[test]
fn integer_subtraction() {
    assert_snapshot!(compile_to_wat("fn sub(a Int, b Int) Int { return a - b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i64)))
      (func (;0;) (type 0) (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.sub
        return
      )
    )
    ");
}

#[test]
fn integer_multiplication() {
    assert_snapshot!(compile_to_wat("fn mul(a Int, b Int) Int { return a * b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i64)))
      (func (;0;) (type 0) (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.mul
        return
      )
    )
    ");
}

#[test]
fn integer_division() {
    assert_snapshot!(compile_to_wat("fn div(a Int, b Int) Int { return a / b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i64)))
      (func (;0;) (type 0) (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.div_s
        return
      )
    )
    ");
}

#[test]
fn integer_modulo() {
    assert_snapshot!(compile_to_wat("fn rem(a Int, b Int) Int { return a % b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i64)))
      (func (;0;) (type 0) (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.rem_s
        return
      )
    )
    ");
}

#[test]
fn integer_negation() {
    // Unary `-x` on an Int compiles to `i64.const 0; local.get x; i64.sub`.
    assert_snapshot!(compile_to_wat("fn neg(x Int) Int { return -x }"), @"
    (module
      (type (;0;) (func (param i64) (result i64)))
      (func (;0;) (type 0) (param i64) (result i64)
        i64.const 0
        local.get 0
        i64.sub
        return
      )
    )
    ");
}

// ── Float arithmetic ──────────────────────────────────────────────────────────

#[test]
fn float_addition() {
    assert_snapshot!(compile_to_wat("fn add(a Float, b Float) Float { return a + b }"), @"
    (module
      (type (;0;) (func (param f64 f64) (result f64)))
      (func (;0;) (type 0) (param f64 f64) (result f64)
        local.get 0
        local.get 1
        f64.add
        return
      )
    )
    ");
}

#[test]
fn float_subtraction() {
    assert_snapshot!(compile_to_wat("fn sub(a Float, b Float) Float { return a - b }"), @"
    (module
      (type (;0;) (func (param f64 f64) (result f64)))
      (func (;0;) (type 0) (param f64 f64) (result f64)
        local.get 0
        local.get 1
        f64.sub
        return
      )
    )
    ");
}

#[test]
fn float_multiplication() {
    assert_snapshot!(compile_to_wat("fn mul(a Float, b Float) Float { return a * b }"), @"
    (module
      (type (;0;) (func (param f64 f64) (result f64)))
      (func (;0;) (type 0) (param f64 f64) (result f64)
        local.get 0
        local.get 1
        f64.mul
        return
      )
    )
    ");
}

#[test]
fn float_division() {
    assert_snapshot!(compile_to_wat("fn div(a Float, b Float) Float { return a / b }"), @"
    (module
      (type (;0;) (func (param f64 f64) (result f64)))
      (func (;0;) (type 0) (param f64 f64) (result f64)
        local.get 0
        local.get 1
        f64.div
        return
      )
    )
    ");
}

#[test]
fn float_negation() {
    // Unary `-x` on a Float compiles directly to `f64.neg`.
    assert_snapshot!(compile_to_wat("fn neg(x Float) Float { return -x }"), @"
    (module
      (type (;0;) (func (param f64) (result f64)))
      (func (;0;) (type 0) (param f64) (result f64)
        local.get 0
        f64.neg
        return
      )
    )
    ");
}

// ── Integer comparisons ───────────────────────────────────────────────────────

#[test]
fn integer_equality() {
    // Comparison operators produce Bool (i32), not Int.
    assert_snapshot!(compile_to_wat("fn eq(a Int, b Int) Bool { return a == b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i32)))
      (func (;0;) (type 0) (param i64 i64) (result i32)
        local.get 0
        local.get 1
        i64.eq
        return
      )
    )
    ");
}

#[test]
fn integer_inequality() {
    assert_snapshot!(compile_to_wat("fn ne(a Int, b Int) Bool { return a != b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i32)))
      (func (;0;) (type 0) (param i64 i64) (result i32)
        local.get 0
        local.get 1
        i64.ne
        return
      )
    )
    ");
}

#[test]
fn integer_less_than() {
    assert_snapshot!(compile_to_wat("fn lt(a Int, b Int) Bool { return a < b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i32)))
      (func (;0;) (type 0) (param i64 i64) (result i32)
        local.get 0
        local.get 1
        i64.lt_s
        return
      )
    )
    ");
}

#[test]
fn integer_greater_than() {
    assert_snapshot!(compile_to_wat("fn gt(a Int, b Int) Bool { return a > b }"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i32)))
      (func (;0;) (type 0) (param i64 i64) (result i32)
        local.get 0
        local.get 1
        i64.gt_s
        return
      )
    )
    ");
}

// ── Float comparisons ─────────────────────────────────────────────────────────

#[test]
fn float_equality() {
    assert_snapshot!(compile_to_wat("fn eq(a Float, b Float) Bool { return a == b }"), @"
    (module
      (type (;0;) (func (param f64 f64) (result i32)))
      (func (;0;) (type 0) (param f64 f64) (result i32)
        local.get 0
        local.get 1
        f64.eq
        return
      )
    )
    ");
}

#[test]
fn float_less_than() {
    assert_snapshot!(compile_to_wat("fn lt(a Float, b Float) Bool { return a < b }"), @"
    (module
      (type (;0;) (func (param f64 f64) (result i32)))
      (func (;0;) (type 0) (param f64 f64) (result i32)
        local.get 0
        local.get 1
        f64.lt
        return
      )
    )
    ");
}

// ── Local variables ───────────────────────────────────────────────────────────

#[test]
fn local_variable_declaration() {
    // A `let` binding introduces an extra Wasm local; the initialiser is
    // stored with `local.set` and retrieved with `local.get`.
    let src = r#"
        fn main() Int {
            let x Int = 42
            return x
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (result i64)))
      (func (;0;) (type 0) (result i64)
        (local i64)
        i64.const 42
        local.set 0
        local.get 0
        return
      )
    )
    ");
}

#[test]
fn variable_reassignment() {
    // Assignment (`x = …`) re-uses the same local slot as the declaration.
    let src = r#"
        fn main() Int {
            let x Int = 1
            x = 2
            return x
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (result i64)))
      (func (;0;) (type 0) (result i64)
        (local i64)
        i64.const 1
        local.set 0
        i64.const 2
        local.set 0
        local.get 0
        return
      )
    )
    ");
}

#[test]
fn declaration_without_initializer_emits_no_store() {
    // A `let` binding with no initializer still reserves a local slot, but emits
    // no `local.set` — there is nothing to store.
    let src = r#"
        fn main() Int {
            let x Int
            return 0
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (result i64)))
      (func (;0;) (type 0) (result i64)
        (local i64)
        i64.const 0
        return
      )
    )
    ");
}

#[test]
fn declaration_without_initializer_then_assigned() {
    // The reserved slot is written by a later assignment, identical to the slot
    // an initialised declaration would have used.
    let src = r#"
        fn main() Int {
            let x Int
            x = 7
            return x
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (result i64)))
      (func (;0;) (type 0) (result i64)
        (local i64)
        i64.const 7
        local.set 0
        local.get 0
        return
      )
    )
    ");
}

#[test]
fn params_and_locals_share_index_space() {
    // Parameters occupy the first N local slots; `let` variables follow.
    let src = r#"
        fn bump(x Int) Int {
            let y Int = 1
            return x + y
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (param i64) (result i64)))
      (func (;0;) (type 0) (param i64) (result i64)
        (local i64)
        i64.const 1
        local.set 1
        local.get 0
        local.get 1
        i64.add
        return
      )
    )
    ");
}

// ── Function calls ────────────────────────────────────────────────────────────

#[test]
fn function_call() {
    // The called function occupies type/func index 0; the caller is index 1.
    let src = r#"
        fn double(x Int) Int { return x + x }
        fn main() Int { return double(5) }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (param i64) (result i64)))
      (type (;1;) (func (result i64)))
      (func (;0;) (type 0) (param i64) (result i64)
        local.get 0
        local.get 0
        i64.add
        return
      )
      (func (;1;) (type 1) (result i64)
        i64.const 5
        call 0
        return
      )
    )
    ");
}

#[test]
fn function_call_not_yet_declared() {
    let src = r#"
        fn main() Int { return double(5) }
        fn double(x Int) Int { return x + x }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (result i64)))
      (type (;1;) (func (param i64) (result i64)))
      (func (;0;) (type 0) (result i64)
        i64.const 5
        call 1
        return
      )
      (func (;1;) (type 1) (param i64) (result i64)
        local.get 0
        local.get 0
        i64.add
        return
      )
    )
    ");
}

// ── Export ────────────────────────────────────────────────────────────────────

#[test]
fn exported_function() {
    // `export fn` adds an entry to the Wasm export section.
    assert_snapshot!(compile_to_wat("export fn add(a Int, b Int) Int { return a + b }"), @r#"
    (module
      (type (;0;) (func (param i64 i64) (result i64)))
      (export "add" (func 0))
      (func (;0;) (type 0) (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.add
        return
      )
    )
    "#);
}

// ── Import ────────────────────────────────────────────────────────────────────

#[test]
fn imported_function() {
    // `import fn` adds an entry to the Wasm import section under module "js".
    assert_snapshot!(compile_to_wat("import fn add(a Int, b Int) Int"), @"
    (module
      (type (;0;) (func (param i64 i64) (result i64)))
      (import \"js\" \"add\" (func (;0;) (type 0)))
    )
    ");
}

#[test]
fn import_and_call() {
    // Imports are assigned func indices before defined functions, so the
    // import gets index 0 and the call instruction references it.
    let src = r#"
        import fn log(n Int)
        fn main() { log(42) }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (param i64)))
      (type (;1;) (func))
      (import \"js\" \"log\" (func (;0;) (type 0)))
      (func (;1;) (type 1)
        i64.const 42
        call 0
      )
    )
    ");
}

// ── Logical operators ─────────────────────────────────────────────────────────

#[test]
fn logical_operators() {
    // `and` short-circuits via if { right } else { false }.
    // `or`  short-circuits via if { true  } else { right }.
    // Both accept and return Bool (i32).
    let src = r#"
        fn and_op(a Bool, b Bool) Bool { return a and b }
        fn or_op(a Bool, b Bool) Bool { return a or b }
        fn not_op(a Bool) Bool { return not a }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (param i32 i32) (result i32)))
      (type (;1;) (func (param i32 i32) (result i32)))
      (type (;2;) (func (param i32) (result i32)))
      (func (;0;) (type 0) (param i32 i32) (result i32)
        local.get 0
        if (result i32) ;; label = @1
          local.get 1
        else
          i32.const 0
        end
        return
      )
      (func (;1;) (type 1) (param i32 i32) (result i32)
        local.get 0
        if (result i32) ;; label = @1
          i32.const 1
        else
          local.get 1
        end
        return
      )
      (func (;2;) (type 2) (param i32) (result i32)
        local.get 0
        i32.eqz
        return
      )
    )
    ");
}

// ── `if` expression compilation ───────────────────────────────────────────────

#[test]
fn if_expression_compiles_to_if_else_result() {
    let wat = compile_to_wat("fn f(a Int) Int { return if a > 0 { 1 } else { 2 } }");
    assert_snapshot!(wat, @"
    (module
      (type (;0;) (func (param i64) (result i64)))
      (func (;0;) (type 0) (param i64) (result i64)
        local.get 0
        i64.const 0
        i64.gt_s
        if (result i64) ;; label = @1
          i64.const 1
        else
          i64.const 2
        end
        return
      )
    )
    ");
}

#[test]
fn if_expression_colon_form() {
    let wat = compile_to_wat("fn f(a Int) Int { return if a > 0: 1 else: 2 }");
    assert_snapshot!(wat, @"
    (module
      (type (;0;) (func (param i64) (result i64)))
      (func (;0;) (type 0) (param i64) (result i64)
        local.get 0
        i64.const 0
        i64.gt_s
        if (result i64) ;; label = @1
          i64.const 1
        else
          i64.const 2
        end
        return
      )
    )
    ");
}

#[test]
fn if_statement_without_else_has_no_result() {
    let src = r#"
        import fn print_int(x Int)
        export fn main() {
            if 1 > 0 {
                print_int(1)
            }
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @r#"
    (module
      (type (;0;) (func (param i64)))
      (type (;1;) (func))
      (import "js" "print_int" (func (;0;) (type 0)))
      (export "main" (func 1))
      (func (;1;) (type 1)
        i64.const 1
        i64.const 0
        i64.gt_s
        if ;; label = @1
          i64.const 1
          call 0
        end
      )
    )
    "#);
}

// ── `while` statement compilation ─────────────────────────────────────────────

#[test]
fn while_compiles_to_block_loop_br_if() {
    let src = r#"
        import fn print_int(x Int)
        export fn main() {
            let a Int = 3
            while a > 0 {
                print_int(a)
                a = a - 1
            }
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @r#"
    (module
      (type (;0;) (func (param i64)))
      (type (;1;) (func))
      (import "js" "print_int" (func (;0;) (type 0)))
      (export "main" (func 1))
      (func (;1;) (type 1)
        (local i64)
        i64.const 3
        local.set 0
        block ;; label = @1
          loop ;; label = @2
            local.get 0
            i64.const 0
            i64.gt_s
            i32.eqz
            br_if 1 (;@1;)
            local.get 0
            call 0
            local.get 0
            i64.const 1
            i64.sub
            local.set 0
            br 0 (;@2;)
          end
        end
      )
    )
    "#);
}

#[test]
fn while_colon_body_drops_value() {
    // The colon body is a single expression; its value must be dropped so the
    // loop's operand stack stays balanced.
    let src = r#"
        export fn main() {
            let a Int = 0
            while a > 0: a
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @r#"
    (module
      (type (;0;) (func))
      (export "main" (func 0))
      (func (;0;) (type 0)
        (local i64)
        i64.const 0
        local.set 0
        block ;; label = @1
          loop ;; label = @2
            local.get 0
            i64.const 0
            i64.gt_s
            i32.eqz
            br_if 1 (;@1;)
            local.get 0
            drop
            br 0 (;@2;)
          end
        end
      )
    )
    "#);
}

// ── `break` / `continue` compilation ──────────────────────────────────────────

#[test]
fn break_branches_out_of_loop_block() {
    // `break` at the top of the loop body branches to the outer `block`
    // (label 1 from inside the `loop`).
    let src = r#"
        export fn main() {
            let a Int = 0
            while a > 0 {
                break
            }
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @r#"
    (module
      (type (;0;) (func))
      (export "main" (func 0))
      (func (;0;) (type 0)
        (local i64)
        i64.const 0
        local.set 0
        block ;; label = @1
          loop ;; label = @2
            local.get 0
            i64.const 0
            i64.gt_s
            i32.eqz
            br_if 1 (;@1;)
            br 1 (;@1;)
            br 0 (;@2;)
          end
        end
      )
    )
    "#);
}

#[test]
fn continue_branches_to_loop() {
    // `continue` jumps back to the `loop` header (label 0).
    let src = r#"
        export fn main() {
            let a Int = 0
            while a > 0 {
                continue
            }
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @r#"
    (module
      (type (;0;) (func))
      (export "main" (func 0))
      (func (;0;) (type 0)
        (local i64)
        i64.const 0
        local.set 0
        block ;; label = @1
          loop ;; label = @2
            local.get 0
            i64.const 0
            i64.gt_s
            i32.eqz
            br_if 1 (;@1;)
            br 0 (;@2;)
            br 0 (;@2;)
          end
        end
      )
    )
    "#);
}

#[test]
fn break_nested_in_if_uses_deeper_label() {
    // From inside `block`/`loop`/`if`, `break` must branch two frames out
    // (label 2) and `continue` one frame out (label 1).
    let src = r#"
        export fn main() {
            let a Int = 0
            while a > 0 {
                if a > 5 {
                    break
                } else {
                    continue
                }
            }
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @r#"
    (module
      (type (;0;) (func))
      (export "main" (func 0))
      (func (;0;) (type 0)
        (local i64)
        i64.const 0
        local.set 0
        block ;; label = @1
          loop ;; label = @2
            local.get 0
            i64.const 0
            i64.gt_s
            i32.eqz
            br_if 1 (;@1;)
            local.get 0
            i64.const 5
            i64.gt_s
            if ;; label = @3
              br 2 (;@1;)
            else
              br 1 (;@2;)
            end
            br 0 (;@2;)
          end
        end
      )
    )
    "#);
}

// ── `return` expression compilation ───────────────────────────────────────────

#[test]
fn return_emits_return_instruction() {
    // A `return` lowers to evaluating the value followed by a Wasm `return`.
    assert_snapshot!(compile_to_wat("fn f(a Int) Int { return a + 1 }"), @"
    (module
      (type (;0;) (func (param i64) (result i64)))
      (func (;0;) (type 0) (param i64) (result i64)
        local.get 0
        i64.const 1
        i64.add
        return
      )
    )
    ");
}

#[test]
fn early_return_in_if_branch() {
    // The early `return` inside the `if` returns from the function; the trailing
    // `return` provides the value on the fall-through path.
    let src = r#"
        fn f(a Int) Int {
            if a > 0 { 
                return 1 
            } 
            return 2
        }
    "#;
    assert_snapshot!(compile_to_wat(src), @"
    (module
      (type (;0;) (func (param i64) (result i64)))
      (func (;0;) (type 0) (param i64) (result i64)
        local.get 0
        i64.const 0
        i64.gt_s
        if ;; label = @1
          i64.const 1
          return
        end
        i64.const 2
        return
      )
    )
    ");
}

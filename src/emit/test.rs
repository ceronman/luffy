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
        Err(e) => format!("PARSE ERROR: {}", pretty::annotate_error_single(src, &e)),
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

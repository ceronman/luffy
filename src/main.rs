pub mod ast;
pub mod compiler;
pub mod emit;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod pretty;
pub mod semantic;

fn main() {
    let code = r#"
        fn foo() {
            let x Int = 1
            let y Int = 2
            let a Float = 1.0
            let b Float = 2.0

            let r1 Float = a + b
            let r2 Int = x + y
            let r3 Float = -a

            let b1 Bool = x > y
        }
    "#;

    let module = match parser::parse(code) {
        Ok(module) => module,
        Err(err) => {
            eprintln!("Error:\n{}", pretty::annotate_error(code, &err));
            return;
        }
    };

    let pretty = pretty::ast::module(&module).expect("Failed to pretty-print AST");
    println!("{pretty}");

    let semantics = match semantic::semantic_analysis(&module) {
        Ok(semantics) => semantics,
        Err(err) => {
            eprintln!("Error:\n{}", pretty::annotate_error(code, &err));
            return;
        }
    };

    let module = match compiler::compile(&module, semantics) {
        Ok(module) => module,
        Err(err) => {
            eprintln!("Error:\n{}", pretty::annotate_error(code, &err));
            return;
        }
    };
    let wasm = emit::emit(module);
    let wat = wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary");
    println!("Running:\n{}", wat);
    emit::run(&wasm).unwrap();
}

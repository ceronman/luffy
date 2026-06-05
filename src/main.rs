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
        fn and_op(a Bool, b Bool) Bool { return a and b }
        fn or_op(a Bool, b Bool) Bool { return a or b }
        fn not_op(a Bool) Bool { not a }
        export fn main() {}
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

    let module = compiler::compile(&module, semantics);
    let wasm = emit::emit(module);
    let wat = wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary");
    println!("Running:\n{}", wat);
    emit::run(&wasm).unwrap();
}

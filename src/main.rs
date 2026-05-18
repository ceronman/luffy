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
    fn f(a Int, b Int) Int {
        let c Int = a + 1
        return c + b
    }
    "#;

    let module = match parser::parse(code) {
        Ok(module) => module,
        Err(err) => {
            let annotated = pretty::annotate_error(code, &err);
            eprintln!("{}", annotated);
            return;
        }
    };

    let pretty = pretty::ast::print(&module).expect("Failed to pretty-print AST");
    println!("{pretty}");

    let semantics = match semantic::semantic_analysis(&module) {
        Ok(semantics) => semantics,
        Err(err) => {
            let annotated = pretty::annotate_error(code, &err);
            eprintln!("Resolve Error: {}", annotated);
            return;
        }
    };

    let module = match compiler::compile(&module, semantics) {
        Ok(module) => module,
        Err(err) => {
            let annotated = pretty::annotate_error(code, &err);
            eprintln!("Compilation Error: {}", annotated);
            return;
        }
    };
    let wasm = emit::emit(module);
    let wat = wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary");
    println!("Running:\n{}", wat);
    emit::run(&wasm).unwrap();
}

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
        let c Int = -a
        c = a + 1
        c = a
        let x Int = 0
        let y Float = 1.0
        x = y
        return g(c, b)
    }

    fn g(a Int, b Int) Int {
        return (a + b) + (b - b) + (b / b) + (a % 2)
    }
    "#;

    let module = match parser::parse(code) {
        Ok(module) => module,
        Err(err) => {
            pretty::print_error(code, &err);
            return;
        }
    };

    let pretty = pretty::ast::print(&module).expect("Failed to pretty-print AST");
    println!("{pretty}");

    let semantics = match semantic::semantic_analysis(&module) {
        Ok(semantics) => semantics,
        Err(err) => {
            pretty::print_error(code, &err);
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

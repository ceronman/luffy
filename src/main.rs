pub mod ast;
pub mod emit;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod pretty;

fn main() {
    let code = r#"
    fn add(a Int, b Int) Int {
        return a + b
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
    let wasm = emit::emit();
    let wat = wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary");
    println!("Running:\n{}", wat);
    emit::run(&wasm).unwrap();
}

pub mod ast;
pub mod compiler;
pub mod emit;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod pretty;
pub mod semantic;
pub mod source;

fn main() {
    let code = r#"
        import fn print_int(x Int)

        export fn main() {
            let foo Array[Array[Int]] = [[1, 2], [10, 20], [100, 200]]
            print_int(foo[0][0])
            print_int(foo[2][1])
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

    let module = compiler::compile(&module, semantics);
    let wasm = emit::emit(module);
    std::fs::write("output.bin", &wasm).unwrap();
    let wat = wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary");
    println!("Running:\n{}", wat);
    emit::run(&wasm).unwrap();
}

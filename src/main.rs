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

        struct Foo {
            a Int
            b Float
            c Bool
            d Bar
        }

        struct Bar {
            x Array[Int]
        }

        export fn main() {
            let x Foo = { a = 1, b = 1.0, c = true, d = { x = [1, 2] } }
            let y Int = 1
            print_int(x.d.x[y])
            x.c = false
            x.a = 100
            print_int(x.a)
            x.d.x[y] = 42
            print_int(x.d.x[y])
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

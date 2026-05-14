pub mod ast;
pub mod emit;
pub mod error;
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
            let annotated = pretty::annotate(code, &err);
            eprintln!("{}", annotated);
            return;
        }
    };

    println!("{:#?}", module);

    let wasm = emit::emit();
    emit::run(&wasm).unwrap();
}

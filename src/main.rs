pub mod ast;
pub mod emit;
pub mod error;
pub mod lexer;
pub mod parser;

fn main() {
    let code = r#"
    fn main() {
        let a Int = 1
        let b Float = 1.0
        let c Bool = true
        print_int(a)
    }
    "#;

    let module = parser::parse(code).expect("parse error");
    println!("{:#?}", module);

    let wasm = emit::emit();
    emit::run(&wasm).unwrap();
}

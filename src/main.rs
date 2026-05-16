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

    let module = ir::Module {
        types: vec![ir::Type::Function(ir::FuncType {
            params: vec![ir::ValType::I64, ir::ValType::I64],
            results: vec![ir::ValType::I64],
        })],
        functions: vec![ir::Function {
            ty: 0,
            body: vec![
                ir::Instruction::LocalGet(0),
                ir::Instruction::LocalGet(1),
                ir::Instruction::I64Add,
                ir::Instruction::End,
            ],
        }],
        exports: vec![ir::Export {
            name: "f".to_string(),
            kind: ir::ExportKind::Func(0),
        }],
    };

    let wasm = emit::emit(module);
    let wat = wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary");
    println!("Running:\n{}", wat);
    emit::run(&wasm).unwrap();
}

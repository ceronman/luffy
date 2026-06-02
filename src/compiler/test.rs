use crate::{compiler, emit, parser, pretty, semantic};
use insta::assert_snapshot;

fn compile_to_wat(src: &str) -> String {
    match parser::parse(src) {
        Ok(module) => match semantic::semantic_analysis(&module) {
            Ok(semantics) => {
                let module = compiler::compile(&module, semantics);
                let wasm = emit::emit(module);
                wasmprinter::print_bytes(&wasm).expect("Failed to print Wasm binary")
            }
            Err(e) => pretty::annotate_error_single(src, &e),
        },
        Err(e) => format!("PARSE ERROR: {}", pretty::annotate_error_single(src, &e)),
    }
}

#[test]
fn simple_function() {
    let wat = compile_to_wat(r#"fn main() Int { 1 + 1 }"#);
    assert_snapshot!(wat, @"
    (module
      (type (;0;) (func (result i64)))
      (func (;0;) (type 0) (result i64)
        i64.const 1
        i64.const 1
        i64.add
      )
    )
    ")
}

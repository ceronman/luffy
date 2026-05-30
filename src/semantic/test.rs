use crate::{parser, pretty, semantic};
use insta::assert_snapshot;

fn check_module(src: &str) -> String {
    match parser::parse(src) {
        Ok(module) => match semantic::semantic_analysis(&module) {
            Ok(_semantics) => "<no error>".to_string(),
            Err(err) => pretty::annotate_error_single(src, &err),
        },
        Err(e) => {
            format!("PARSE ERROR: {}", pretty::annotate_error_single(src, &e))
        }
    }
}

#[test]
fn function_incorrect_return_type() {
    let src = "fn main() { return 0 }";
    assert_snapshot!(check_module(src), @"
    fn main() { return 0 }
                       ^ ─── Type mismatch: expected 'Unit', found 'Int'
    ");
}

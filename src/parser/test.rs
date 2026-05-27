use crate::ast::StmtKind;
use crate::{parser, pretty};
use insta::assert_snapshot;

#[test]
fn function() {
    let src = r#"
        fn add(a Int, b Int) Int {
            return a + b
        }
    "#;
    assert_snapshot!(parse_module(src), @"
    Module
    └── Function [add]
        ├── Parameters
        │   ├── Param
        │   │   ├── Name
        │   │   │   └── a
        │   │   └── Type
        │   │       └── Int
        │   └── Param
        │       ├── Name
        │       │   └── b
        │       └── Type
        │           └── Int
        └── Body
            └── Block
                └── Return
                    └── Binary [+]
                        ├── Var [a]
                        └── Var [b]
    ");
}

#[test]
fn precedence() {
    assert_snapshot!(parse_expr("1 + 2 * 3"), @"
    Binary [+]
    ├── Int[1]
    └── Binary [*]
        ├── Int[2]
        └── Int[3]
    ");
}

#[test]
fn grouping() {
    assert_snapshot!(parse_expr("(1 + 2) * 3"), @"
    Binary [*]
    ├── Binary [+]
    │   ├── Int[1]
    │   └── Int[2]
    └── Int[3]
    ");
}

fn parse_module(src: &str) -> String {
    match parser::parse(src) {
        Ok(m) => pretty::ast::module(&m).unwrap(),
        Err(e) => pretty::error(src, &e),
    }
}

fn parse_expr(expr: &str) -> String {
    let module = format!("fn foo() {{\n{expr}\n}}");
    match parser::parse(&module) {
        Ok(m) => {
            let StmtKind::Block { statements } = &m.items[0].body.kind else {
                return pretty::ast::module(&m).unwrap();
            };
            let StmtKind::ExprStmt { expr: e } = &statements[0].kind else {
                return pretty::ast::module(&m).unwrap();
            };
            pretty::ast::expr(e).unwrap()
        }
        Err(e) => pretty::error(&module, &e),
    }
}

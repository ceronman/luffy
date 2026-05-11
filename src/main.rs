use crate::lexer::{Lexer, TokenKind};

pub mod lexer;

fn main() {
    let code = r#"
        let a Int = 1
        let b Float = 1.0
        let c Bool = true
    "#;

    let mut lexer = Lexer::new(code);
    loop {
        let token = lexer.next_token();
        if let TokenKind::Eof = token.kind {
            break;
        }
        println!("{:?}", token);
    }
}

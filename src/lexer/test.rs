use crate::lexer::{Lexer, Token, TokenKind};
use insta::assert_snapshot;
use std::fmt::{Display, Formatter};

#[test]
fn operators() {
    assert_snapshot!(
        tokens(" + - / * % == != < > >= <= ! ="),
        @"
    [ `+`  `-`  `/`  `*`  `%`  `==`  `!=`  `<`  `>`  `>=`  `<=` 
     `!`  `=` ]
    "
    );
}

#[test]
fn braces() {
    assert_snapshot!(
        tokens("{ [ ( ) ] }"),
        @"[ `{`  `[`  `(`  `)`  `]`  `}` ]"
    );
}

#[test]
fn keywords() {
    assert_snapshot!(
        tokens("true false let if else while fn return"),
        @"[ `true`  `false`  `let`  `if`  `else`  `while`  `fn`  `return` ]"
    );
}

#[test]
fn import_export_keywords() {
    assert_snapshot!(
        tokens("import export"),
        @"[ `import`  `export` ]"
    );
}

#[test]
fn identifiers() {
    assert_snapshot!(
        tokens("one two three foo bar"),
        @"[ <Identifier>  <Identifier>  <Identifier>  <Identifier>  <Identifier> ]"
    );
}

#[test]
fn punctuations() {
    assert_snapshot!(
        tokens("... ,,,"),
        @"[ `.`  `.`  `.`  `,`  `,`  `,` ]"
    );
}

#[test]
fn numbers() {
    assert_snapshot!(
        tokens("1 2 1000 -1 1.0 0.1"),
        @"[ `<Int>  `<Int>  `<Int>  `-`  `<Int>  <Float>  <Float> ]"
    );
}

#[test]
fn trivial() {
    assert_snapshot!(
        tokens_with_trivial(r#"
            // This is a line comment
            /* This is a block comment */
            /* This is a /*single*/ comment (no nesting) */
        "#),
        @"
    [ <End of line>  <Whitespace>  <Line comment>  <End of line>  <Whitespace> 
     <Block comment>  <End of line>  <Whitespace>  <Block comment> 
     <End of line>  <Whitespace> ]
    "
    );
}

struct Tokens(Vec<Token>);

fn tokens(input: &str) -> Tokens {
    let mut result = Vec::new();
    let mut lexer = Lexer::new(input);
    loop {
        let token = lexer.next_non_trivial();
        if let TokenKind::Eol = token.kind {
            continue;
        }
        if let TokenKind::Eof = token.kind {
            break;
        }
        result.push(token);
    }
    Tokens(result)
}

fn tokens_with_trivial(input: &str) -> Tokens {
    let mut result = Vec::new();
    let mut lexer = Lexer::new(input);
    loop {
        let token = lexer.next_token();
        if let TokenKind::Eof = token.kind {
            break;
        }
        result.push(token);
    }
    Tokens(result)
}

impl Display for Tokens {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[",)?;
        let mut width = 0;
        for token in &self.0 {
            let s = format!("{}", token.kind);
            if width + s.len() > 80 {
                writeln!(f)?;
                width = 0;
            }
            write!(f, " {} ", token.kind)?;
            width += s.len() + 4;
        }
        write!(f, "]",)
    }
}

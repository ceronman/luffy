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
fn logical_keywords() {
    assert_snapshot!(
        tokens("and or not"),
        @"[ `and`  `or`  `not` ]"
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
        tokens("... ,,, :"),
        @"[ `.`  `.`  `.`  `,`  `,`  `,`  `:` ]"
    );
}

#[test]
fn semicolon() {
    // New token: used as an explicit statement separator by the parser.
    // Snapshot left empty: run `cargo insta review` to regenerate.
    assert_snapshot!(tokens("; ;;"), @"[ `;`  `;`  `;` ]");
}

#[test]
fn numbers() {
    assert_snapshot!(
        tokens("1 2 1000 -1 1.0 0.1"),
        @"[ `<Int>  `<Int>  `<Int>  `-`  `<Int>  <Float>  <Float> ]"
    );
}

#[test]
fn hexadecimal_numbers() {
    // Each literal is lexed as a single `Int` token; the parser interprets it.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("0xFF 0xff 0xDE_AD 0X1a"), @"[ `<Int>  `<Int>  `<Int>  `<Int> ]");
}

#[test]
fn octal_numbers() {
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("0o17 0o755 0O1_7"), @"[ `<Int>  `<Int>  `<Int> ]");
}

#[test]
fn binary_numbers() {
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("0b101 0b1010_0101 0B11"), @"[ `<Int>  `<Int>  `<Int> ]");
}

#[test]
fn underscore_separated_numbers() {
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("1_000 1_000_000 1_000.000_5"), @"[ `<Int>  `<Int>  <Float> ]");
}

#[test]
fn malformed_numbers_are_single_tokens() {
    // The lexer greedily captures a base-prefixed literal as one token even
    // when the digits are illegal for the base; the parser reports the error.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("0o18 0xGG 0b12"), @"[ `<Int>  `<Int>  `<Int> ]");
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
        let token = lexer.next_significant();
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

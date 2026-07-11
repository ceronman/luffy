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
fn break_and_continue_keywords() {
    assert_snapshot!(
        tokens("break continue"),
        @"[ `break`  `continue` ]"
    );
}

#[test]
fn eof_token_at_end_of_input() {
    let mut lexer = Lexer::new("break");
    assert_eq!(lexer.next_significant().kind, TokenKind::Break);
    assert_eq!(lexer.next_significant().kind, TokenKind::Eof);
    assert_eq!(lexer.next_significant().kind, TokenKind::Eof);
}

#[test]
fn import_export_keywords() {
    assert_snapshot!(
        tokens("import export"),
        @"[ `import`  `export` ]"
    );
}

#[test]
fn keyword_struct() {
    assert_snapshot!(
        tokens("struct"),
        @"[ `struct` ]"
    );
}

#[test]
fn struct_declaration_tokens() {
    // A struct declaration lexes into the `struct` keyword, the name and field
    // identifiers, the braces, and the field type identifiers.
    assert_snapshot!(
        tokens("struct Foo { a Int }"),
        @"[ `struct`  <Identifier>  `{`  <Identifier>  <Identifier>  `}` ]"
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
fn float_leading_dot() {
    // `.5` is a single Float token; `.` alone is still the dot token.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens(".5 .25 .000_5"), @"[ <Float>  <Float>  <Float> ]");
}

#[test]
fn float_scientific_notation() {
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("1e10 1E10 1.5e-3 2E+8 .5e3 10.e3"), @"[ <Float>  <Float>  <Float>  <Float>  <Float>  <Float> ]");
}

#[test]
fn dot_is_still_a_token_without_following_digit() {
    // A `.` not followed by a digit must remain the standalone dot token.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("a . b ... 1 . 2"), @"
    [ <Identifier>  `.`  <Identifier>  `.`  `.`  `.`  `<Int>  `.` 
     `<Int> ]
    ");
}

#[test]
fn bare_e_is_not_an_exponent() {
    // `e`/`E` only starts an exponent when followed by digits, so `2e` lexes
    // as the integer `2` and the identifier `e`.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("2e 1e+ x"), @"[ `<Int>  <Identifier>  `<Int>  <Identifier>  `+`  <Identifier> ]");
}

#[test]
fn malformed_numbers_are_single_tokens() {
    // The lexer greedily captures a base-prefixed literal as one token even
    // when the digits are illegal for the base; the parser reports the error.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens("0o18 0xGG 0b12"), @"[ `<Int>  `<Int>  `<Int> ]");
}

#[test]
fn strings() {
    // The lexer only recognizes the token's extent; escape sequences are
    // validated and decoded later, by the parser.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens(r#""hello" "" "with spaces""#), @"[ <String>  <String>  <String> ]");
}

#[test]
fn string_with_escaped_quote_does_not_end_the_token() {
    // `\"` must not be treated as the closing quote: this is a single Str
    // token, not `<String>` followed by garbage.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens(r#""a\"b" 1"#), @"[ <String>  `<Int> ]");
}

#[test]
fn string_with_escaped_backslash_before_closing_quote() {
    // `\\` immediately before the closing `"` must not cause the quote to be
    // swallowed as an escape (the backslash escapes itself, not the quote).
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens(r#""a\\" 1"#), @"[ <String>  `<Int> ]");
}

#[test]
fn unterminated_string_is_an_error_token() {
    // Reaching EOF before a closing `"` is an error, not a silently
    // truncated string.
    // Snapshot left empty: run `cargo insta test && cargo insta review`.
    assert_snapshot!(tokens(r#""unterminated"#), @"[ <Error> ]");
}

#[test]
fn unterminated_block_comment_is_an_error_token() {
    assert_snapshot!(tokens("/* unterminated"), @"[ <Error> ]");
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

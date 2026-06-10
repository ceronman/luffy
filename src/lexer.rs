#[cfg(test)]
mod test;

use crate::source::Span;
use std::fmt::Display;
use std::str::Chars;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Plus,
    Minus,
    Slash,
    Star,
    Percent,

    EqualEqual,
    BangEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    And,
    Or,
    Not,

    Equal,
    Bang,

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    True,
    False,
    Int,
    Float,
    Identifier,
    Str,

    Let,
    If,
    Else,
    While,
    Break,
    Continue,
    Fn,
    Return,
    Import,
    Export,

    LineComment,
    BlockComment,

    Whitespace,
    Eol,
    Comma,
    Dot,

    Colon,
    Semicolon,

    Eof,
    Error,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// Whether at least one end-of-line token was skipped immediately before
    /// this token while scanning for the next significant token. The parser
    /// uses this (Swift-style `isAtStartOfLine`) to detect statement
    /// boundaries without newlines appearing in its token stream.
    pub newline_before: bool,
}

#[derive(Clone)]
pub struct Lexer<'src> {
    source: &'src str,
    chars: Chars<'src>,
    start: usize,
    offset: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            chars: source.chars(),
            start: 0,
            offset: 0,
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.start = self.offset;
        let c = self.eat();

        Token {
            kind: self.token_kind(c),
            span: Span {
                start: self.start,
                end: self.offset,
            },
            newline_before: false,
        }
    }

    /// Returns the next token that is significant to the parser, skipping
    /// whitespace, comments, and end-of-line tokens.
    pub fn next_significant(&mut self) -> Token {
        let mut newline_before = false;
        loop {
            let mut token = self.next_token();
            match token.kind {
                TokenKind::Whitespace | TokenKind::BlockComment | TokenKind::LineComment => {
                    continue;
                }
                TokenKind::Eol => {
                    newline_before = true;
                    continue;
                }
                _ => {
                    token.newline_before = newline_before;
                    return token;
                }
            }
        }
    }

    fn token_kind(&mut self, c: Option<char>) -> TokenKind {
        let Some(c) = c else { return TokenKind::Eof };

        match (c, self.peek()) {
            (' ' | '\t' | '\r', _) => self.whitespace(),
            ('\n', _) => self.eol(),
            ('+', _) => TokenKind::Plus,
            ('-', _) => TokenKind::Minus,
            ('*', _) => TokenKind::Star,
            ('/', Some('/')) => self.line_comment(),
            ('/', Some('*')) => self.block_comment(),
            ('/', _) => TokenKind::Slash,
            ('%', _) => TokenKind::Percent,
            ('=', Some('=')) => self.eat_and(TokenKind::EqualEqual),
            ('=', _) => TokenKind::Equal,
            ('!', Some('=')) => self.eat_and(TokenKind::BangEqual),
            ('!', _) => TokenKind::Bang,
            ('>', Some('=')) => self.eat_and(TokenKind::GreaterEqual),
            ('>', _) => TokenKind::Greater,
            ('<', Some('=')) => self.eat_and(TokenKind::LessEqual),
            ('<', _) => TokenKind::Less,
            ('(', _) => TokenKind::LParen,
            (')', _) => TokenKind::RParen,
            ('{', _) => TokenKind::LBrace,
            ('}', _) => TokenKind::RBrace,
            ('[', _) => TokenKind::LBracket,
            (']', _) => TokenKind::RBracket,
            (',', _) => TokenKind::Comma,
            ('.', Some('0'..='9')) => self.dot_number(),
            ('.', _) => TokenKind::Dot,
            (':', _) => TokenKind::Colon,
            (';', _) => TokenKind::Semicolon,
            ('0'..='9', _) => self.number(c),
            ('"', _) => self.string(),
            (c, _) if c == '_' || c.is_alphabetic() => self.identifier(),
            _ => TokenKind::Error,
        }
    }

    fn eol(&mut self) -> TokenKind {
        while let Some('\n' | '\r') = self.peek() {
            self.eat();
        }
        TokenKind::Eol
    }

    fn whitespace(&mut self) -> TokenKind {
        while let Some(' ' | '\t' | '\r') = self.peek() {
            self.eat();
        }
        TokenKind::Whitespace
    }

    fn line_comment(&mut self) -> TokenKind {
        self.eat(); // Consume second '/'
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.eat();
        }
        TokenKind::LineComment
    }

    fn block_comment(&mut self) -> TokenKind {
        self.eat(); // Consume '*'
        let mut level = 1;
        while let Some(c) = self.eat() {
            if c == '*' && matches!(self.peek(), Some('/')) {
                self.eat();
                level -= 1;
            }
            if c == '/' && matches!(self.peek(), Some('*')) {
                self.eat();
                level += 1;
            }
            if level == 0 {
                break;
            }
        }
        if level == 0 {
            TokenKind::BlockComment
        } else {
            TokenKind::Error
        }
    }

    /// Scans a numeric literal. The lexer only recognizes the *shape* of the
    /// literal and emits a span-only `Int` or `Float` token; the parser turns
    /// the text into a value.
    ///
    /// Recognized integer forms:
    ///   - decimal:     `42`, `1_000`
    ///   - hexadecimal: `0xFF`, `0xDE_AD`
    ///   - octal:       `0o17`
    ///   - binary:      `0b1010_0101`
    ///
    /// Recognized float forms (decimal only, Python-style):
    ///   - fractional:  `3.14`, `10.`, `1_000.5`
    ///   - scientific:  `1e10`, `1.5e-3`, `2E+8`
    ///     (leading-dot floats like `.5` enter through `dot_number`)
    ///
    /// `first` is the first digit, already consumed.
    fn number(&mut self, first: char) -> TokenKind {
        // Base-prefixed integers: 0x.., 0o.., 0b..
        if first == '0'
            && let Some('x' | 'X' | 'o' | 'O' | 'b' | 'B') = self.peek()
        {
            self.eat(); // consume the base prefix letter
            while let Some(c) = self.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    self.eat();
                } else {
                    break;
                }
            }
            return TokenKind::Int;
        }

        // Decimal integer part (digits and `_` separators).
        self.decimal_digits();

        // A fractional part and/or an exponent makes this a float.
        let mut is_float = false;
        if let Some('.') = self.peek() {
            is_float = true;
            self.eat();
            self.decimal_digits();
        }
        if self.maybe_exponent() {
            is_float = true;
        }

        if is_float {
            TokenKind::Float
        } else {
            TokenKind::Int
        }
    }

    fn dot_number(&mut self) -> TokenKind {
        self.decimal_digits();
        self.maybe_exponent();
        TokenKind::Float
    }

    fn maybe_exponent(&mut self) -> bool {
        if !self.has_exponent() {
            return false;
        }
        self.eat(); // 'e' or 'E'
        if let Some('+' | '-') = self.peek() {
            self.eat();
        }
        self.decimal_digits();
        true
    }

    fn has_exponent(&self) -> bool {
        let mut chars = self.chars.clone();
        match chars.next() {
            Some('e' | 'E') => {}
            _ => return false,
        }
        let mut next = chars.next();
        if let Some('+' | '-') = next {
            next = chars.next();
        }
        matches!(next, Some('0'..='9'))
    }

    fn decimal_digits(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '_' {
                self.eat();
            } else {
                break;
            }
        }
    }

    fn string(&mut self) -> TokenKind {
        //TODO: Handle unterminated string!
        while let Some(c) = self.eat() {
            if c == '"' {
                break;
            }
        }
        TokenKind::Str
    }

    fn identifier(&mut self) -> TokenKind {
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.eat();
            } else {
                break;
            }
        }
        match &self.source[self.start..self.offset] {
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "import" => TokenKind::Import,
            "export" => TokenKind::Export,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            _ => TokenKind::Identifier,
        }
    }

    fn eat(&mut self) -> Option<char> {
        if let Some(c) = self.chars.next() {
            self.offset += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }

    fn eat_and(&mut self, token_kind: TokenKind) -> TokenKind {
        self.eat();
        token_kind
    }

    fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let result = match self {
            TokenKind::Plus => "`+`",
            TokenKind::Minus => "`-`",
            TokenKind::Slash => "`/`",
            TokenKind::Star => "`*`",
            TokenKind::Percent => "`%`",
            TokenKind::EqualEqual => "`==`",
            TokenKind::BangEqual => "`!=`",
            TokenKind::Greater => "`>`",
            TokenKind::GreaterEqual => "`>=`",
            TokenKind::Less => "`<`",
            TokenKind::LessEqual => "`<=`",
            TokenKind::Equal => "`=`",
            TokenKind::Bang => "`!`",
            TokenKind::And => "`and`",
            TokenKind::Or => "`or`",
            TokenKind::Not => "`not`",
            TokenKind::LParen => "`(`",
            TokenKind::RParen => "`)`",
            TokenKind::LBrace => "`{`",
            TokenKind::RBrace => "`}`",
            TokenKind::LBracket => "`[`",
            TokenKind::RBracket => "`]`",
            TokenKind::True => "`true`",
            TokenKind::False => "`false`",
            TokenKind::Int => "`<Int>",
            TokenKind::Float => "<Float>",
            TokenKind::Identifier => "<Identifier>",
            TokenKind::Str => "<String>",
            TokenKind::Let => "`let`",
            TokenKind::If => "`if`",
            TokenKind::Else => "`else`",
            TokenKind::While => "`while`",
            TokenKind::Break => "`break`",
            TokenKind::Continue => "`continue`",
            TokenKind::Fn => "`fn`",
            TokenKind::Return => "`return`",
            TokenKind::Import => "`import`",
            TokenKind::Export => "`export`",
            TokenKind::LineComment => "<Line comment>",
            TokenKind::BlockComment => "<Block comment>",
            TokenKind::Whitespace => "<Whitespace>",
            TokenKind::Eol => "<End of line>",
            TokenKind::Comma => "`,`",
            TokenKind::Dot => "`.`",
            TokenKind::Colon => "`:`",
            TokenKind::Semicolon => "`;`",
            TokenKind::Eof => "<End of file>",
            TokenKind::Error => "<Error>",
        };
        write!(f, "{result}")
    }
}

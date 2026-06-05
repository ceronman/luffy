#[cfg(test)]
mod test;

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

    Eof,
    Error,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
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
        }
    }

    pub fn next_non_trivial(&mut self) -> Token {
        loop {
            let token = self.next_token();
            match token.kind {
                TokenKind::Whitespace | TokenKind::BlockComment | TokenKind::LineComment => {
                    continue;
                }
                _ => return token,
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
            ('.', _) => TokenKind::Dot,
            ('0'..='9', _) => self.number(),
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

    fn number(&mut self) -> TokenKind {
        // TODO: implement floats starting with dot and proper notation.
        while let Some('0'..='9') = self.peek() {
            self.eat();
        }
        if let Some('.') = self.peek() {
            self.eat();
            while let Some('0'..='9') = self.peek() {
                self.eat();
            }
            TokenKind::Float
        } else {
            TokenKind::Int
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
            TokenKind::Eof => "<End of file>",
            TokenKind::Error => "<Error>",
        };
        write!(f, "{result}")
    }
}

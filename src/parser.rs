#[cfg(test)]
mod test;

use crate::ast::StmtKind::{Assignment, ExprStmt};
use crate::ast::{
    BinOp, BinOpKind, Expr, ExprKind, Identifier, Item, ItemKind, LiteralKind, Module, Node, Param,
    Stmt, StmtKind, TypeKind, TypeRef, UnOp, UnOpKind,
};
use crate::error::{CompilerError, ErrorKind};
use crate::lexer::{Lexer, Span, Token, TokenKind};
use std::fmt::Debug;

struct Parser<'src> {
    source: &'src str,
    current: Token,
    lexer: Lexer<'src>,
    id_counter: u32,
}

pub type Result<T> = std::result::Result<T, CompilerError>;

fn error<T: Debug>(span: Span, message: impl Into<String>) -> Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Parse,
        msg: message.into(),
        span,
    })
}

impl<'src> Parser<'src> {
    fn new(source: &'src str) -> Self {
        let mut lexer = Lexer::new(source);
        Parser {
            source,
            current: lexer.next_token(),
            lexer,
            id_counter: 0,
        }
    }

    fn module(&mut self) -> Result<Module> {
        self.maybe_eol();

        let begin = self.current.span;
        let mut end = begin;
        let mut items = Vec::new();

        while self.current.kind != TokenKind::Eof {
            let item = self.item()?;
            end = item.node.span;
            items.push(item);
            self.maybe_eol();
        }

        Ok(Module {
            node: self.node(begin, end),
            items,
        })
    }

    fn item(&mut self) -> Result<Item> {
        match self.current.kind {
            TokenKind::Export | TokenKind::Fn => self.function(),
            TokenKind::Import => self.import(),
            _ => error(
                self.current.span,
                format!(
                    "Expected item declaration, found {} instead",
                    self.current.kind
                ),
            ),
        }
    }

    fn function(&mut self) -> Result<Item> {
        let begin = self.current.span;
        let export = self.eat(TokenKind::Export);
        self.expect(TokenKind::Fn)?;
        // TODO: handle new lines between
        let name =
            self.identifier(|t| format!("Expected function name, found {} instead", t.kind))?;
        self.expect(TokenKind::LParen)?;
        let params = self.params()?;
        self.expect(TokenKind::RParen)?;
        let return_ty = if self.current.kind == TokenKind::Identifier {
            Some(self.type_ref()?)
        } else {
            None
        };
        let body = self.statement()?;
        Ok(Item {
            node: self.node(begin, body.node.span),
            kind: ItemKind::Function {
                export,
                name,
                return_ty,
                params,
                body,
            },
        })
    }

    fn import(&mut self) -> Result<Item> {
        let begin = self.expect(TokenKind::Import)?.span;
        self.expect(TokenKind::Fn)?;
        // TODO: handle new lines between
        let name =
            self.identifier(|t| format!("Expected function name, found {} instead", t.kind))?;
        self.expect(TokenKind::LParen)?;
        let params = self.params()?;
        let mut end = self.expect(TokenKind::RParen)?.span;
        let return_ty = if self.current.kind == TokenKind::Identifier {
            let type_ref = self.type_ref()?;
            end = type_ref.node.span;
            Some(type_ref)
        } else {
            None
        };
        Ok(Item {
            node: self.node(begin, end),
            kind: ItemKind::Import {
                name,
                return_ty,
                params,
            },
        })
    }

    fn params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        if self.current.kind != TokenKind::RParen {
            loop {
                self.maybe_eol();
                params.push(self.param()?);

                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        Ok(params)
    }

    fn identifier(&mut self, msg: impl Fn(Token) -> String) -> Result<Identifier> {
        let token = self.expect_msg(TokenKind::Identifier, msg)?;
        let symbol = self.slice(token.span).into();
        Ok(Identifier {
            node: self.node(token.span, token.span),
            symbol,
        })
    }

    fn type_ref(&mut self) -> Result<TypeRef> {
        let name = self.identifier(|t| format!("Expected type, found {} instead", t.kind))?;
        let kind = match name.symbol.as_str() {
            "Int" => TypeKind::Int,
            "Float" => TypeKind::Float,
            "Bool" => TypeKind::Bool,
            _ => return error(name.node.span, "Unknown type"),
        };
        Ok(TypeRef {
            node: self.node(name.node.span, name.node.span),
            kind,
        })
    }

    fn param(&mut self) -> Result<Param> {
        let name = self.identifier(|t| format!("Expected param name, found {} instead", t.kind))?;
        let ty = self.type_ref()?;
        Ok(Param {
            node: self.node(name.node.span, ty.node.span),
            name,
            ty,
        })
    }

    fn statement(&mut self) -> Result<Stmt> {
        self.maybe_eol();
        match self.current.kind {
            TokenKind::LBrace => self.block(),
            TokenKind::Let => self.declaration(),
            TokenKind::Return => self.return_statement(),
            _ => self.expr_stmt(),
        }
    }

    fn expression_precedence(&mut self, min_precedence: u8) -> Result<Expr> {
        let mut prefix = match self.current.kind {
            TokenKind::LParen => self.grouping()?,
            TokenKind::Int | TokenKind::Float | TokenKind::False | TokenKind::True => {
                self.literal()?
            }
            TokenKind::Identifier => self.variable()?,
            TokenKind::Minus => self.unary_expr()?,

            other_kind => {
                return error(self.current.span, format!("Unexpected {}", other_kind));
            }
        };

        while let Some(precedence) = self.infix_precedence() {
            if precedence < min_precedence {
                break;
            }
            prefix = match self.current.kind {
                TokenKind::LParen => self.call(prefix)?,
                _ => self.binary_expr(prefix, precedence)?,
            };
        }

        Ok(prefix)
    }

    fn grouping(&mut self) -> Result<Expr> {
        let lparen = self.expect(TokenKind::LParen)?;
        let inner = self.expression()?;
        let rparen = self.expect(TokenKind::RParen)?;
        Ok(Expr {
            node: self.node(lparen.span, rparen.span),
            kind: inner.kind,
        })
    }

    fn unary_expr(&mut self) -> Result<Expr> {
        let Some(precedence) = self.prefix_precedence() else {
            return error(self.current.span, "Unknown infix precedence");
        };
        let operator = self.unary_operator()?;
        let right = self.expression_precedence(precedence)?;
        Ok(Expr {
            node: self.node(operator.node.span, right.node.span),
            kind: ExprKind::Unary {
                op: operator,
                expr: right.into(),
            },
        })
    }

    fn call(&mut self, prefix: Expr) -> Result<Expr> {
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        if self.current.kind != TokenKind::RParen {
            loop {
                self.maybe_eol();
                args.push(self.expression()?);

                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        let rparen = self.expect(TokenKind::RParen)?;
        Ok(Expr {
            node: self.node(prefix.node.span, rparen.span),
            kind: ExprKind::Call {
                callee: prefix.into(),
                args,
            },
        })
    }

    fn binary_expr(&mut self, prefix: Expr, precedence: u8) -> Result<Expr> {
        let operator = self.binary_operator()?;
        let right = self.expression_precedence(precedence + 1)?;
        Ok(Expr {
            node: self.node(prefix.node.span, right.node.span),
            kind: ExprKind::Binary {
                left: prefix.into(),
                right: right.into(),
                op: operator,
            },
        })
    }

    fn binary_operator(&mut self) -> Result<BinOp> {
        let op = self.current;
        let kind = match op.kind {
            TokenKind::Plus => BinOpKind::Add,
            TokenKind::Minus => BinOpKind::Sub,
            TokenKind::Star => BinOpKind::Mul,
            TokenKind::Slash => BinOpKind::Div,
            TokenKind::Percent => BinOpKind::Mod,
            TokenKind::EqualEqual => BinOpKind::Eq,
            TokenKind::BangEqual => BinOpKind::Ne,
            TokenKind::Less => BinOpKind::Lt,
            TokenKind::LessEqual => BinOpKind::Le,
            TokenKind::Greater => BinOpKind::Gt,
            TokenKind::GreaterEqual => BinOpKind::Ge,
            TokenKind::And => BinOpKind::And,
            TokenKind::Or => BinOpKind::Or,
            _ => return error(op.span, "Invalid binary operator {op:?}"),
        };
        self.advance();
        Ok(BinOp {
            node: self.node(op.span, op.span),
            kind,
        })
    }

    fn unary_operator(&mut self) -> Result<UnOp> {
        let op = self.current;
        let kind = match op.kind {
            TokenKind::Minus => UnOpKind::Neg,
            _ => return error(op.span, "Invalid unary operator {op:?}"),
        };
        self.advance();
        Ok(UnOp {
            node: self.node(op.span, op.span),
            kind,
        })
    }

    #[rustfmt::skip]
    fn prefix_precedence(&self) -> Option<u8> {
        use TokenKind::*;
        match self.current.kind {
            Minus                   => Some(7),
            _                       => None
        }
    }

    #[rustfmt::skip]
    fn infix_precedence(&self) -> Option<u8> {
        use TokenKind::*;
        match self.current.kind {
            Dot | LParen            => Some(8),
            LBracket                => Some(7),
            Star | Slash | Percent  => Some(6),
            Plus | Minus            => Some(5),
            Greater | GreaterEqual
            | Less | LessEqual      => Some(4),
            EqualEqual | BangEqual  => Some(3),
            And                     => Some(2),
            Or                      => Some(1),
            _                       => None
        }
    }

    fn expression(&mut self) -> Result<Expr> {
        self.expression_precedence(0)
    }

    fn literal(&mut self) -> Result<Expr> {
        let token = self.current;
        let kind = match token.kind {
            TokenKind::True => ExprKind::Literal {
                kind: LiteralKind::Bool(true),
            },
            TokenKind::False => ExprKind::Literal {
                kind: LiteralKind::Bool(false),
            },
            TokenKind::Int => {
                let value = self.slice(token.span);
                let value: i64 = match value.parse() {
                    Ok(v) => v,
                    Err(e) => {
                        return error(
                            self.current.span,
                            format!("Unable to parse integer value: {e}"),
                        );
                    }
                };
                ExprKind::Literal {
                    kind: LiteralKind::Int(value),
                }
            }
            TokenKind::Float => {
                let value = self.slice(token.span);
                let value: f64 = match value.parse() {
                    Ok(v) => v,
                    Err(e) => {
                        return error(
                            self.current.span,
                            format!("Unable to parse float value: {e}"),
                        );
                    }
                };
                ExprKind::Literal {
                    kind: LiteralKind::Float(value),
                }
            }
            _ => {
                return error(token.span, format!("Expected literal, got {}", token.kind));
            }
        };
        self.advance();
        Ok(Expr {
            node: self.node(token.span, token.span),
            kind,
        })
    }

    fn variable(&mut self) -> Result<Expr> {
        let name =
            self.identifier(|t| format!("Expected variable name, found {} instead", t.kind))?;
        Ok(Expr {
            node: name.node,
            kind: ExprKind::Variable { name },
        })
    }

    fn expr_stmt(&mut self) -> Result<Stmt> {
        let expr = self.expression()?;

        if self.eat(TokenKind::Equal) {
            let value = self.expression()?;
            Ok(Stmt {
                node: self.node(expr.node.span, value.node.span),
                kind: Assignment {
                    target: expr,
                    value,
                },
            })
        } else {
            Ok(Stmt {
                node: self.node(expr.node.span, expr.node.span),
                kind: ExprStmt { expr },
            })
        }
    }

    fn declaration(&mut self) -> Result<Stmt> {
        let let_kw = self.expect(TokenKind::Let)?;
        let name =
            self.identifier(|t| format!("Expected variable name, found {} instead", t.kind))?;
        let ty = self.type_ref()?;
        self.expect(TokenKind::Equal)?;
        let initializer = self.expression()?;

        Ok(Stmt {
            node: self.node(let_kw.span, initializer.node.span),
            kind: StmtKind::Declaration {
                name,
                ty,
                initializer,
            },
        })
    }

    fn block(&mut self) -> Result<Stmt> {
        let lbrace = self.expect(TokenKind::LBrace)?;
        self.maybe_eol();
        let mut statements = Vec::new();
        while self.current.kind != TokenKind::RBrace {
            statements.push(self.statement()?);
            self.maybe_eol();
        }
        let rbrace = self.expect(TokenKind::RBrace)?;
        self.maybe_eol();
        Ok(Stmt {
            node: self.node(lbrace.span, rbrace.span),
            kind: StmtKind::Block { statements },
        })
    }

    fn return_statement(&mut self) -> Result<Stmt> {
        let return_kw = self.expect(TokenKind::Return)?;
        let expr = self.expression()?;
        Ok(Stmt {
            node: self.node(return_kw.span, expr.node.span),
            kind: StmtKind::Return { expr },
        })
    }

    fn maybe_eol(&mut self) {
        while self.eat(TokenKind::Eol) {}
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_non_trivial();
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.current.kind == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, token_kind: TokenKind) -> Result<Token> {
        self.expect_msg(token_kind, |token| {
            format!("Expected {}, got {}", token_kind, token.kind)
        })
    }

    fn expect_msg(
        &mut self,
        token_kind: TokenKind,
        msg: impl Fn(Token) -> String,
    ) -> Result<Token> {
        let token = self.current;
        if token.kind == token_kind {
            self.advance();
            return Ok(token);
        }
        error(self.current.span, msg(token))
    }

    fn node(&mut self, start: Span, end: Span) -> Node {
        let result = Node {
            id: self.id_counter,
            span: Span {
                start: start.start,
                end: end.end,
            },
        };
        self.id_counter += 1;
        result
    }

    fn slice(&mut self, span: Span) -> &str {
        &self.source[span.start..span.end]
    }
}

pub fn parse(source: &str) -> Result<Module> {
    Parser::new(source).module()
}

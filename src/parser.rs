mod numbers;
#[cfg(test)]
mod test;

use crate::ast::StmtKind::ExprStmt;
use crate::ast::{
    BinOp, BinOpKind, Block, BlockKind, Expr, ExprKind, Identifier, Item, ItemKind, LiteralKind,
    Module, Node, Param, Stmt, StmtKind, TypeArgKind, TypeArgs, TypeRef, UnOp, UnOpKind,
};
use crate::error::{CompilerError, ErrorKind};
use crate::lexer::{Lexer, Token, TokenKind};
use crate::source::Span;
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
            current: lexer.next_significant(),
            lexer,
            id_counter: 0,
        }
    }

    fn module(&mut self) -> Result<Module> {
        let begin = self.current.span;
        let mut end = begin;
        let mut items = Vec::new();

        while self.current.kind != TokenKind::Eof {
            let item = self.item()?;
            end = item.node.span;
            items.push(item);
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
        let body = self.function_body()?;
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
        let args = self.type_args()?;

        Ok(TypeRef {
            node: self.node(name.node.span, name.node.span),
            name,
            args,
        })
    }

    fn type_args(&mut self) -> Result<Vec<TypeArgs>> {
        let mut params = Vec::new();
        if self.eat(TokenKind::LBracket) {
            if self.current.kind != TokenKind::RBracket {
                loop {
                    let begin = self.current.span;
                    let mut end = self.current.span;
                    let kind = match self.current.kind {
                        TokenKind::Int => {
                            let value = self.integer()?;
                            TypeArgKind::Number(value)
                        }
                        TokenKind::Identifier => {
                            let ty_ref = self.type_ref()?;
                            end = ty_ref.node.span;
                            TypeArgKind::Type(ty_ref)
                        }
                        _ => {
                            return error(
                                self.current.span,
                                format!(
                                    "Expected type argument, found {} instead",
                                    self.current.kind
                                ),
                            );
                        }
                    };
                    params.push(TypeArgs {
                        node: self.node(begin, end),
                        kind,
                    });

                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RBracket)?;
        }

        Ok(params)
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
        match self.current.kind {
            TokenKind::Let => self.declaration(),
            TokenKind::While => self.while_statement(),
            _ => self.expr_stmt(),
        }
    }

    fn while_statement(&mut self) -> Result<Stmt> {
        let while_kw = self.expect(TokenKind::While)?;
        let condition = self.expression()?;
        let body = self.block("`while` body")?;
        Ok(Stmt {
            node: self.node(while_kw.span, body.node.span),
            kind: StmtKind::While {
                condition,
                body: body.into(),
            },
        })
    }

    fn expression_precedence(&mut self, min_precedence: u8) -> Result<Expr> {
        let mut prefix = match self.current.kind {
            TokenKind::LParen => self.grouping()?,
            TokenKind::Int | TokenKind::Float | TokenKind::False | TokenKind::True => {
                self.literal()?
            }
            TokenKind::Identifier => self.variable()?,
            TokenKind::Not | TokenKind::Minus => self.unary_expr()?,
            TokenKind::If => self.if_expr()?,
            TokenKind::Break => self.break_expr()?,
            TokenKind::Continue => self.continue_expr()?,
            TokenKind::Return => self.return_expr()?,

            other_kind => {
                return error(self.current.span, format!("Unexpected {}", other_kind));
            }
        };

        while let Some(precedence) = self.infix_precedence() {
            if precedence < min_precedence {
                break;
            }
            // A `(` at the start of a new line begins a new statement rather
            // than continuing this expression as a call (Swift-style). On the
            // same line it is still a call, so multi-line argument lists like
            // `foo(\n  a,\n  b,\n)` keep working.
            if self.current.kind == TokenKind::LParen && self.current.newline_before {
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
            _ => return error(op.span, format!("Invalid binary operator {}", op.kind)),
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
            TokenKind::Not => UnOpKind::Not,
            _ => return error(op.span, format!("Invalid unary operator {}", op.kind)),
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
            Not | Minus             => Some(7),
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
        // Assignment is an expression. It binds looser than every
        // binary operator and is right-associative (`a = b = c` parses as
        // `a = (b = c)`), so we parse a full operator-precedence expression for the
        // target, then recurse on the right side when an `=` follows.
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr> {
        let target = self.expression_precedence(0)?;
        if self.eat(TokenKind::Equal) {
            let value = self.assignment()?;
            Ok(Expr {
                node: self.node(target.node.span, value.node.span),
                kind: ExprKind::Assignment {
                    target: target.into(),
                    value: value.into(),
                },
            })
        } else {
            Ok(target)
        }
    }

    fn literal(&mut self) -> Result<Expr> {
        let token = self.current;
        let kind = match token.kind {
            TokenKind::True => {
                self.advance();
                ExprKind::Literal {
                    kind: LiteralKind::Bool(true),
                }
            }
            TokenKind::False => {
                self.advance();
                ExprKind::Literal {
                    kind: LiteralKind::Bool(false),
                }
            }
            TokenKind::Int => {
                let value = self.integer()?;
                ExprKind::Literal {
                    kind: LiteralKind::Int(value),
                }
            }
            TokenKind::Float => {
                let value = self.float()?;
                ExprKind::Literal {
                    kind: LiteralKind::Float(value),
                }
            }
            _ => {
                return error(token.span, format!("Expected literal, got {}", token.kind));
            }
        };
        Ok(Expr {
            node: self.node(token.span, token.span),
            kind,
        })
    }

    fn integer(&mut self) -> Result<i64> {
        let token = self.expect(TokenKind::Int)?;
        let text = self.slice(token.span);
        let value = match numbers::parse_int_literal(text) {
            Ok(v) => v,
            Err(msg) => return error(token.span, msg),
        };
        Ok(value)
    }

    fn float(&mut self) -> Result<f64> {
        let token = self.expect(TokenKind::Float)?;
        let text = self.slice(token.span);
        let value = match numbers::parse_float_literal(text) {
            Ok(v) => v,
            Err(msg) => return error(token.span, msg),
        };
        Ok(value)
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
        Ok(Stmt {
            node: self.node(expr.node.span, expr.node.span),
            kind: ExprStmt { expr },
        })
    }

    fn declaration(&mut self) -> Result<Stmt> {
        let let_kw = self.expect(TokenKind::Let)?;
        let name =
            self.identifier(|t| format!("Expected variable name, found {} instead", t.kind))?;
        let ty = self.type_ref()?;
        let mut end = name.node.span;
        let initializer = if self.eat(TokenKind::Equal) {
            let expr = self.expression()?;
            end = expr.node.span;
            Some(expr)
        } else {
            None
        };

        Ok(Stmt {
            node: self.node(let_kw.span, end),
            kind: StmtKind::Declaration {
                name,
                ty,
                initializer,
            },
        })
    }

    fn function_body(&mut self) -> Result<Block> {
        self.block("function body")
    }

    fn block(&mut self, context: &str) -> Result<Block> {
        match self.current.kind {
            TokenKind::LBrace => self.braces_block(),
            TokenKind::Colon => self.expr_block(),
            _ => error(
                self.current.span,
                format!(
                    "Expected `{{` or `:` for {context}, found {} instead",
                    self.current.kind
                ),
            ),
        }
    }

    fn braces_block(&mut self) -> Result<Block> {
        let lbrace = self.expect(TokenKind::LBrace)?;
        let mut statements = Vec::new();
        loop {
            self.skip_semicolons();
            if matches!(self.current.kind, TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            statements.push(self.statement()?);
            self.stmt_separator()?;
        }
        let rbrace = self.expect(TokenKind::RBrace)?;
        Ok(Block {
            node: self.node(lbrace.span, rbrace.span),
            kind: BlockKind::Braces { statements },
        })
    }

    fn expr_block(&mut self) -> Result<Block> {
        let colon = self.expect(TokenKind::Colon)?;
        let expr = self.expression()?;
        let end = expr.node.span;
        Ok(Block {
            node: self.node(colon.span, end),
            kind: BlockKind::Expr { expr },
        })
    }

    fn if_expr(&mut self) -> Result<Expr> {
        let if_kw = self.expect(TokenKind::If)?;
        let condition = self.expression()?;
        let then_branch = self.block("`if` branch")?;
        let mut end = then_branch.node.span;
        let else_branch = if self.eat(TokenKind::Else) {
            let block = if self.current.kind == TokenKind::If {
                let expr = self.if_expr()?;
                let span = expr.node.span;
                Block {
                    node: self.node(span, span),
                    kind: BlockKind::Expr { expr },
                }
            } else {
                self.block("`else` branch")?
            };
            end = block.node.span;
            Some(block.into())
        } else {
            None
        };
        Ok(Expr {
            node: self.node(if_kw.span, end),
            kind: ExprKind::If {
                condition: condition.into(),
                then_branch: then_branch.into(),
                else_branch,
            },
        })
    }

    fn break_expr(&mut self) -> Result<Expr> {
        let kw = self.expect(TokenKind::Break)?;
        Ok(Expr {
            node: self.node(kw.span, kw.span),
            kind: ExprKind::Break,
        })
    }

    fn continue_expr(&mut self) -> Result<Expr> {
        let kw = self.expect(TokenKind::Continue)?;
        Ok(Expr {
            node: self.node(kw.span, kw.span),
            kind: ExprKind::Continue,
        })
    }

    fn return_expr(&mut self) -> Result<Expr> {
        let kw = self.expect(TokenKind::Return)?;
        let expr = self.expression()?;
        let end = expr.node.span;
        Ok(Expr {
            node: self.node(kw.span, end),
            kind: ExprKind::Return { expr: expr.into() },
        })
    }

    fn skip_semicolons(&mut self) {
        while self.eat(TokenKind::Semicolon) {}
    }

    fn stmt_separator(&mut self) -> Result<()> {
        if !self.current.newline_before
            && !matches!(
                self.current.kind,
                TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
            )
        {
            return error(
                self.current.span,
                format!(
                    "Expected newline or `;` between statements, found {}",
                    self.current.kind
                ),
            );
        }
        Ok(())
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_significant();
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

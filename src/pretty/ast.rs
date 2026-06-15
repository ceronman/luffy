use crate::ast::{
    BinOpKind, Block, BlockKind, Expr, ExprKind, Identifier, Item, ItemKind, LiteralKind, Module,
    Stmt, StmtKind, TypeRef, UnOpKind,
};
use std::fmt::Write;

struct PrettyAst {
    label: String,
    children: Vec<PrettyAst>,
}

impl PrettyAst {
    fn new(label: impl Into<String>, children: impl IntoIterator<Item = PrettyAst>) -> Self {
        Self {
            label: label.into(),
            children: children.into_iter().collect(),
        }
    }
    fn module(program: &Module) -> Self {
        Self::new("Module", program.items.iter().map(Self::item))
    }
    fn item(item: &Item) -> PrettyAst {
        match &item.kind {
            ItemKind::Function {
                export,
                name,
                params,
                return_ty,
                body,
            } => Self::new(
                format!("Function [{}]", name.symbol),
                vec![
                    Self::new(format!("Export [{export}]"), vec![]),
                    Self::new(
                        "Parameters",
                        params.iter().map(|param| {
                            Self::new(
                                "Param",
                                vec![
                                    Self::new("Name", vec![Self::identifier(&param.name)]),
                                    Self::new("Type", vec![Self::ty(&param.ty)]),
                                ],
                            )
                        }),
                    ),
                    Self::new(
                        "Return",
                        vec![
                            return_ty
                                .as_ref()
                                .map(Self::ty)
                                .unwrap_or(Self::new("Unit", vec![])),
                        ],
                    ),
                    Self::new("Body", vec![Self::block(body)]),
                ],
            ),
            ItemKind::Import {
                name,
                params,
                return_ty,
            } => Self::new(
                format!("Import [{}]", name.symbol),
                vec![
                    Self::new(
                        "Parameters",
                        params.iter().map(|param| {
                            Self::new(
                                "Param",
                                vec![
                                    Self::new("Name", vec![Self::identifier(&param.name)]),
                                    Self::new("Type", vec![Self::ty(&param.ty)]),
                                ],
                            )
                        }),
                    ),
                    Self::new(
                        "Return",
                        vec![
                            return_ty
                                .as_ref()
                                .map(Self::ty)
                                .unwrap_or(Self::new("Unit", vec![])),
                        ],
                    ),
                ],
            ),
        }
    }
    fn block(block: &Block) -> PrettyAst {
        match &block.kind {
            BlockKind::Braces { statements } => {
                Self::new("Block", statements.iter().map(Self::stmt))
            }
            BlockKind::Expr { expr } => Self::new("ExprBlock", vec![Self::expression(expr)]),
        }
    }
    fn stmt(statement: &Stmt) -> PrettyAst {
        match &statement.kind {
            StmtKind::ExprStmt { expr } => Self::expression(expr),
            StmtKind::Declaration {
                name,
                ty,
                initializer,
            } => {
                let mut children = vec![Self::new("Type", vec![Self::ty(ty)])];
                if let Some(value) = initializer {
                    children.push(Self::expression(value));
                }
                Self::new(format!("LetDeclaration [{}]", name.symbol), children)
            }
            StmtKind::While { condition, body } => Self::new(
                "While",
                vec![
                    Self::new("Condition", vec![Self::expression(condition)]),
                    Self::new("Body", vec![Self::block(body)]),
                ],
            ),
        }
    }
    fn expression(expression: &Expr) -> PrettyAst {
        match &expression.kind {
            ExprKind::Literal { kind } => Self::literal(kind),
            ExprKind::Variable { name } => Self::new(format!("Var [{}]", name.symbol), vec![]),
            ExprKind::Unary { op, expr } => Self::new(
                format!("Unary [{}]", Self::unary_op(&op.kind)),
                vec![Self::expression(expr)],
            ),
            ExprKind::Binary { op, left, right } => Self::new(
                format!("Binary [{}]", Self::binary_op(&op.kind)),
                vec![Self::expression(left), Self::expression(right)],
            ),
            ExprKind::Call { callee, args } => {
                let callee = Self::expression(callee);
                let args = Self::new("Args", args.iter().map(Self::expression));
                Self::new("Call", vec![callee, args])
            }
            ExprKind::Assignment { target, value } => {
                let target = Self::expression(target);
                let value = Self::expression(value);
                Self::new("Assign", vec![target, value])
            }
            ExprKind::Index { expr, index } => {
                let expr = Self::expression(expr);
                let index = Self::expression(index);
                Self::new("Index", vec![expr, index])
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let mut children = vec![
                    Self::new("Condition", vec![Self::expression(condition)]),
                    Self::new("Then", vec![Self::block(then_branch)]),
                ];
                if let Some(else_branch) = else_branch {
                    children.push(Self::new("Else", vec![Self::block(else_branch)]));
                }
                Self::new("If", children)
            }
            ExprKind::Break => Self::new("Break", vec![]),
            ExprKind::Continue => Self::new("Continue", vec![]),
            ExprKind::Return { expr } => Self::new("Return", vec![Self::expression(expr)]),
        }
    }

    fn identifier(identifier: &Identifier) -> PrettyAst {
        Self::new(&identifier.symbol, vec![])
    }

    fn literal(literal: &LiteralKind) -> PrettyAst {
        match literal {
            LiteralKind::Int(v) => Self::new(format!("Int[{}]", *v), vec![]),
            LiteralKind::Float(v) => Self::new(format!("Float[{}]", *v), vec![]),
            LiteralKind::Bool(v) => Self::new(format!("Bool[{}]", *v), vec![]),
        }
    }

    fn ty(ty: &TypeRef) -> PrettyAst {
        // Only render an `Args` child when the type actually has arguments, so
        // plain (non-generic) types keep their original `Type [Name]` leaf form.
        let children = if ty.args.is_empty() {
            vec![]
        } else {
            vec![Self::new("Args", ty.args.iter().map(Self::ty))]
        };
        Self::new(format!("Type [{}]", ty.name.symbol), children)
    }

    fn write(
        &self,
        stream: &mut impl Write,
        pipe: &str,
        level: usize,
        pipes: &[usize],
    ) -> Result<(), std::fmt::Error> {
        let indent = Self::make_indent(level, pipes);
        writeln!(stream, "{indent}{pipe}{}", self.label)?;
        for (i, child) in self.children.iter().enumerate() {
            if i < self.children.len() - 1 {
                child.write(stream, "├── ", level + 1, &[pipes, &[level + 1]].concat())?;
            } else {
                child.write(stream, "└── ", level + 1, pipes)?;
            }
        }
        Ok(())
    }

    fn make_indent(level: usize, pipes: &[usize]) -> String {
        let mut indent = String::new();
        for l in 1..level {
            if pipes.contains(&l) {
                indent.push_str("│   ");
            } else {
                indent.push_str("    ");
            }
        }
        indent
    }

    fn binary_op(op: &BinOpKind) -> &str {
        match op {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            BinOpKind::Div => "/",
            BinOpKind::Mod => "%",
            BinOpKind::Eq => "==",
            BinOpKind::Ne => "!=",
            BinOpKind::Lt => "<",
            BinOpKind::Le => "<=",
            BinOpKind::Gt => ">",
            BinOpKind::Ge => ">=",
            BinOpKind::And => "and",
            BinOpKind::Or => "or",
        }
    }

    fn unary_op(op: &UnOpKind) -> &str {
        match op {
            UnOpKind::Neg => "-",
            UnOpKind::Not => "not",
        }
    }
}

pub fn module(module: &Module) -> Result<String, std::fmt::Error> {
    let mut buffer = String::new();
    PrettyAst::module(module).write(&mut buffer, "", 0, &[])?;
    Ok(buffer.trim().to_string())
}

pub fn expr(expr: &Expr) -> Result<String, std::fmt::Error> {
    let mut buffer = String::new();
    PrettyAst::expression(expr).write(&mut buffer, "", 0, &[])?;
    Ok(buffer.trim().to_string())
}

pub fn stmt(stmt: &Stmt) -> Result<String, std::fmt::Error> {
    let mut buffer = String::new();
    PrettyAst::stmt(stmt).write(&mut buffer, "", 0, &[])?;
    Ok(buffer.trim().to_string())
}

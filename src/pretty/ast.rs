use crate::ast::{
    BinOpKind, Expr, ExprKind, Function, Identifier, LiteralKind, Module, Stmt, StmtKind, TypeKind,
    TypeRef, UnOpKind,
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
        Self::new("Module", program.items.iter().map(Self::function))
    }
    fn function(function: &Function) -> PrettyAst {
        let mut children = Vec::new();

        if !function.params.is_empty() {
            children.push(Self::new(
                "Parameters",
                function.params.iter().map(|param| {
                    Self::new(
                        "Param",
                        vec![
                            Self::new("Name", vec![Self::identifier(&param.name)]),
                            Self::new("Type", vec![Self::ty(&param.ty)]),
                        ],
                    )
                }),
            ))
        }
        children.push(Self::new("Body", vec![Self::stmt(&function.body)]));
        Self::new(format!("Function [{}]", function.name.symbol), children)
    }
    fn stmt(statement: &Stmt) -> PrettyAst {
        match &statement.kind {
            StmtKind::Return { expr } => Self::new("Return", vec![Self::expression(expr)]),
            StmtKind::ExprStmt { expr } => Self::expression(expr),
            StmtKind::Block { statements } => Self::new("Block", statements.iter().map(Self::stmt)),
            StmtKind::Declaration {
                name,
                ty,
                initializer: value,
            } => {
                let ty = Self::new("Type", vec![Self::ty(ty)]);
                Self::new(
                    format!("VarDeclaration [{}]", name.symbol),
                    vec![ty, Self::expression(value)],
                )
            }
            StmtKind::Assignment { target, value } => {
                let target = Self::expression(target);
                let value = Self::expression(value);
                Self::new("Assign", vec![target, value])
            }
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
        match ty.kind {
            TypeKind::Int => Self::new("Int", vec![]),
            TypeKind::Float => Self::new("Float", vec![]),
            TypeKind::Bool => Self::new("Bool", vec![]),
        }
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
        }
    }

    fn unary_op(op: &UnOpKind) -> &str {
        match op {
            UnOpKind::Neg => "-",
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

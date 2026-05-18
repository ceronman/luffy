use crate::lexer::Span; // TODO: Maybe move to separate module

pub type NodeId = u32;
pub type Symbol = String;

#[derive(Copy, Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub node: Node,
    pub symbol: Symbol,
}

#[derive(Debug)]
pub struct Module {
    pub node: Node,
    pub items: Vec<Function>,
}

#[derive(Debug)]
pub struct Function {
    pub node: Node,
    pub name: Identifier,
    pub return_ty: Option<Type>,
    pub params: Vec<Param>,
    pub body: Stmt,
}

#[derive(Debug)]
pub struct Type {
    pub node: Node,
    pub kind: TypeKind,
}

#[derive(Debug)]
pub enum TypeKind {
    Int,
    Float,
    Bool,
}

#[derive(Debug)]
pub struct Param {
    pub node: Node,
    pub name: Identifier,
    pub ty: Type,
}

#[derive(Debug)]
pub struct Stmt {
    pub node: Node,
    pub kind: StmtKind,
}

#[derive(Debug)]
pub enum StmtKind {
    ExprStmt {
        expr: Expr,
    },
    Block {
        statements: Vec<Stmt>,
    },
    Declaration {
        name: Identifier,
        ty: Type,
        initializer: Expr,
    },
    Assignment {
        target: Expr,
        value: Expr,
    },
    Return {
        expr: Expr,
    },
}

#[derive(Debug)]
pub struct Expr {
    pub node: Node,
    pub kind: ExprKind,
}

#[derive(Debug)]
pub enum ExprKind {
    Literal {
        kind: LiteralKind,
    },
    Variable {
        name: Identifier,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
}

#[derive(Debug)]
pub enum LiteralKind {
    Int(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Debug)]
pub struct UnOp {
    pub node: Node,
    pub kind: UnOpKind,
}

#[derive(Debug)]
pub enum UnOpKind {
    Neg,
}

#[derive(Debug)]
pub struct BinOp {
    pub node: Node,
    pub kind: BinOpKind,
}

#[derive(Debug)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

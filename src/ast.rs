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
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub struct Item {
    pub node: Node,
    pub kind: ItemKind,
}

#[derive(Debug)]
pub enum ItemKind {
    Function {
        export: bool,
        name: Identifier,
        params: Vec<Param>,
        return_ty: Option<TypeRef>,
        body: Block,
    },
    Import {
        name: Identifier,
        params: Vec<Param>,
        return_ty: Option<TypeRef>,
    },
}

#[derive(Debug)]
pub struct TypeRef {
    pub node: Node,
    pub name: Identifier,
}

#[derive(Debug)]
pub struct Param {
    pub node: Node,
    pub name: Identifier,
    pub ty: TypeRef,
}

#[derive(Debug)]
pub struct Block {
    pub node: Node,
    pub kind: BlockKind,
}

#[derive(Debug)]
pub enum BlockKind {
    Braces { statements: Vec<Stmt> },
    Expr { expr: Expr },
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
    Declaration {
        name: Identifier,
        ty: TypeRef,
        initializer: Expr,
    },
    Assignment {
        target: Expr,
        value: Expr,
    },
    While {
        condition: Expr,
        body: Box<Block>,
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
    If {
        condition: Box<Expr>,
        then_branch: Box<Block>,
        else_branch: Option<Box<Block>>,
    },
    Break,
    Continue,
    Return {
        expr: Box<Expr>,
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
    Not,
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
    And,
    Or,
}

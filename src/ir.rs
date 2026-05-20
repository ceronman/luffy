pub type Idx = u32;
pub type TypeIdx = Idx;
pub type LocalIdx = Idx;
pub type FuncIdx = Idx;

pub struct Module {
    pub types: Vec<Type>,
    pub functions: Vec<Function>,
    pub exports: Vec<Export>,
}

pub enum Type {
    Function(FuncType),
}

pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

pub enum ValType {
    I64,
    F64,
}

pub struct Function {
    pub ty: TypeIdx,
    pub locals: Vec<ValType>,
    pub body: Vec<Instruction>,
}

pub enum Instruction {
    I64Const(i64),
    I64Add,
    I64Sub,
    LocalGet(LocalIdx),
    End,
    LocalSet(LocalIdx),
    Call(FuncIdx),
}

pub struct Export {
    pub name: String,
    pub kind: ExportKind,
}

pub enum ExportKind {
    Function(FuncIdx),
}

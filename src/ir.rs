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
    pub body: Vec<Instruction>,
}

pub enum Instruction {
    I64Add,
    LocalGet(LocalIdx),
    End,
}

pub struct Export {
    pub name: String,
    pub kind: ExportKind,
}

pub enum ExportKind {
    Function(FuncIdx),
}

pub type Idx = u32;
pub type TypeIdx = Idx;
pub type LocalIdx = Idx;
pub type FuncIdx = Idx;
pub type LabelIdx = Idx;

pub struct Module {
    pub types: Vec<Type>,
    pub imports: Vec<Import>,
    pub functions: Vec<Function>,
    pub exports: Vec<Export>,
}

#[derive(Clone)]
pub enum Type {
    Function {
        params: Vec<ValType>,
        results: Vec<ValType>,
    },
    Array {
        ty: StorageType,
    },
}

#[derive(Copy, Clone)]
pub enum ValType {
    I64,
    F64,
    I32,
    Ref(TypeIdx),
}

#[derive(Clone)]
pub enum StorageType {
    I8,
    I16,
    Val(ValType),
}

pub struct Function {
    pub ty: TypeIdx,
    pub locals: Vec<ValType>,
    pub body: Vec<Instruction>,
}

pub enum BlockType {
    Empty,
    Result(ValType),
    FunctionType(FuncIdx),
}

pub enum Instruction {
    I32Const(i32),
    I32Eqz,
    If(BlockType),
    Else,
    Block(BlockType),
    Loop(BlockType),
    Br(LabelIdx),
    BrIf(LabelIdx),
    Return,

    I64Const(i64),
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64RemS,

    I64Eq,
    I64Ne,
    I64GeS,
    I64GtS,
    I64LeS,
    I64LtS,

    F64Const(f64),
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Neg,

    F64Eq,
    F64Ne,
    F64Ge,
    F64Gt,
    F64Lt,
    F64Le,

    LocalGet(LocalIdx),
    LocalSet(LocalIdx),
    Drop,
    ArrayGet(TypeIdx),
    Call(FuncIdx),
    End,
}

pub struct Export {
    pub name: String, // TODO: change strings to symbols
    pub kind: ExportKind,
}

pub enum ExportKind {
    Function(FuncIdx),
}

pub struct Import {
    pub module: String,
    pub name: String,
    pub func_type: TypeIdx, // TODO: Generalize
}

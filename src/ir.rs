pub type Idx = u32;
pub type TypeIdx = Idx;
pub type LocalIdx = Idx;
pub type FuncIdx = Idx;
pub type LabelIdx = Idx;
pub type FieldIdx = Idx;

pub struct Module {
    pub types: Vec<RecGroup>,
    pub imports: Vec<Import>,
    pub functions: Vec<Function>,
    pub exports: Vec<Export>,
}

#[derive(Clone)]
pub struct RecGroup {
    pub types: Vec<Type>,
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
    Struct {
        fields: Vec<StorageType>,
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
    I32WrapI64,

    I32Eq,
    I32Ne,
    I32GeU,
    I32GtU,
    I32LeU,
    I32LtU,

    If(BlockType),
    Else,
    Block(BlockType),
    Loop(BlockType),
    Br(LabelIdx),
    BrIf(LabelIdx),
    Return,
    Unreachable,

    I64Const(i64),
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64RemS,
    I64DivU,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,
    I64Clz,
    I64Ctz,
    I64Popcnt,

    I64Eq,
    I64Ne,
    I64GeS,
    I64GtS,
    I64LeS,
    I64LtS,
    I64GtU,

    I64ExtendI32U,
    I64TruncF64S,

    F64Const(f64),
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Neg,
    F64Sqrt,
    F64Abs,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Min,
    F64Max,
    F64Copysign,
    F64ConvertI64S,

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
    /// Read from an array of packed (`i8`/`i16`) elements, zero-extending.
    ArrayGetU(TypeIdx),
    ArraySet(TypeIdx),
    ArrayNewFixed {
        type_idx: TypeIdx,
        len: u32,
    },

    StructNew(TypeIdx),
    StructGet {
        type_idx: TypeIdx,
        field_idx: u32,
    },
    StructSet {
        type_idx: TypeIdx,
        field_idx: u32,
    },

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

#[cfg(test)]
mod test;

use crate::ir;
use wasm_encoder::{
    ArrayType, BlockType, CodeSection, CompositeInnerType, CompositeType, EntityType, ExportKind,
    ExportSection, FieldType, FuncType, Function, FunctionSection, HeapType, ImportSection,
    Instruction, Module, RefType, StorageType, StructType, SubType, TypeSection, ValType,
};

pub fn emit(module: ir::Module) -> Vec<u8> {
    let mut bin_module = Module::new();

    let mut types = TypeSection::new();
    for group in &module.types {
        if let [ty] = group.types.as_slice() {
            types.ty().subtype(&ty.encode());
        } else {
            types.ty().rec(group.types.iter().map(ir::Type::encode));
        }
    }
    bin_module.section(&types);

    let mut imports = ImportSection::new();
    for import in &module.imports {
        imports.import(
            &import.module,
            &import.name,
            EntityType::Function(import.func_type),
        );
    }
    bin_module.section(&imports);

    let mut functions = FunctionSection::new();
    let mut codes = CodeSection::new();
    for f in &module.functions {
        functions.function(f.ty);
        let locals = f.locals.iter().map(|ty| (1, ty.encode()));
        let mut bin_function = Function::new(locals);
        for instruction in &f.body {
            bin_function.instruction(&instruction.encode());
        }
        codes.function(&bin_function);
    }
    bin_module.section(&functions);

    let mut exports = ExportSection::new();
    for e in &module.exports {
        match e.kind {
            ir::ExportKind::Function(fn_idx) => {
                exports.export(&e.name, ExportKind::Func, fn_idx);
            }
        }
    }
    bin_module.section(&exports);
    bin_module.section(&codes);

    bin_module.finish()
}

impl ir::Type {
    fn encode(&self) -> SubType {
        let inner = match self {
            ir::Type::Function { params, results } => CompositeInnerType::Func(FuncType::new(
                params.iter().map(ir::ValType::encode),
                results.iter().map(ir::ValType::encode),
            )),
            ir::Type::Array { ty } => CompositeInnerType::Array(ArrayType(FieldType {
                element_type: ty.encode(),
                mutable: true,
            })),
            ir::Type::Struct { fields } => CompositeInnerType::Struct(StructType {
                fields: fields
                    .iter()
                    .map(|f| FieldType {
                        element_type: f.encode(),
                        mutable: true,
                    })
                    .collect(),
            }),
        };
        SubType {
            is_final: true,
            supertype_idx: None,
            composite_type: CompositeType {
                inner,
                shared: false,
                descriptor: None,
                describes: None,
            },
        }
    }
}

impl ir::ValType {
    fn encode(&self) -> ValType {
        match self {
            ir::ValType::I64 => ValType::I64,
            ir::ValType::F64 => ValType::F64,
            ir::ValType::I32 => ValType::I32,
            ir::ValType::Ref(idx) => ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(*idx),
            }),
        }
    }
}

impl ir::StorageType {
    fn encode(&self) -> StorageType {
        match self {
            ir::StorageType::I8 => StorageType::I8,
            ir::StorageType::I16 => StorageType::I16,
            ir::StorageType::Val(v) => StorageType::Val(v.encode()),
        }
    }
}

impl ir::BlockType {
    fn encode(&self) -> BlockType {
        match self {
            ir::BlockType::Empty => BlockType::Empty,
            ir::BlockType::Result(v) => BlockType::Result(v.encode()),
            ir::BlockType::FunctionType(idx) => BlockType::FunctionType(*idx),
        }
    }
}

impl ir::Instruction {
    fn encode(&self) -> Instruction<'_> {
        match self {
            ir::Instruction::LocalGet(idx) => Instruction::LocalGet(*idx),
            ir::Instruction::LocalSet(idx) => Instruction::LocalSet(*idx),
            ir::Instruction::Drop => Instruction::Drop,
            ir::Instruction::ArrayGet(idx) => Instruction::ArrayGet(*idx),
            ir::Instruction::ArraySet(idx) => Instruction::ArraySet(*idx),
            ir::Instruction::ArrayNewFixed { type_idx, len } => Instruction::ArrayNewFixed {
                array_type_index: *type_idx,
                array_size: *len,
            },
            ir::Instruction::StructNew(idx) => Instruction::StructNew(*idx),
            ir::Instruction::StructSet {
                type_idx,
                field_idx,
            } => Instruction::StructSet {
                struct_type_index: *type_idx,
                field_index: *field_idx,
            },
            ir::Instruction::StructGet {
                type_idx,
                field_idx,
            } => Instruction::StructGet {
                struct_type_index: *type_idx,
                field_index: *field_idx,
            },
            ir::Instruction::I64Const(v) => Instruction::I64Const(*v),
            ir::Instruction::I64Add => Instruction::I64Add,
            ir::Instruction::I64Sub => Instruction::I64Sub,
            ir::Instruction::I64Mul => Instruction::I64Mul,
            ir::Instruction::I64DivS => Instruction::I64DivS,
            ir::Instruction::I64RemS => Instruction::I64RemS,
            ir::Instruction::I64Eq => Instruction::I64Eq,
            ir::Instruction::I64Ne => Instruction::I64Ne,
            ir::Instruction::I64GeS => Instruction::I64GeS,
            ir::Instruction::I64GtS => Instruction::I64GtS,
            ir::Instruction::I64LeS => Instruction::I64LeS,
            ir::Instruction::I64LtS => Instruction::I64LtS,

            ir::Instruction::F64Const(v) => Instruction::F64Const((*v).into()),
            ir::Instruction::F64Add => Instruction::F64Add,
            ir::Instruction::F64Sub => Instruction::F64Sub,
            ir::Instruction::F64Mul => Instruction::F64Mul,
            ir::Instruction::F64Div => Instruction::F64Div,
            ir::Instruction::F64Neg => Instruction::F64Neg,
            ir::Instruction::F64Eq => Instruction::F64Eq,
            ir::Instruction::F64Ne => Instruction::F64Ne,
            ir::Instruction::F64Ge => Instruction::F64Ge,
            ir::Instruction::F64Gt => Instruction::F64Gt,
            ir::Instruction::F64Le => Instruction::F64Le,
            ir::Instruction::F64Lt => Instruction::F64Lt,

            ir::Instruction::I32Const(v) => Instruction::I32Const(*v),
            ir::Instruction::I32Eqz => Instruction::I32Eqz,
            ir::Instruction::I32WrapI64 => Instruction::I32WrapI64,
            ir::Instruction::If(block_type) => Instruction::If(block_type.encode()),
            ir::Instruction::Else => Instruction::Else,
            ir::Instruction::Block(block_type) => Instruction::Block(block_type.encode()),
            ir::Instruction::Loop(block_type) => Instruction::Loop(block_type.encode()),
            ir::Instruction::Br(label) => Instruction::Br(*label),
            ir::Instruction::BrIf(label) => Instruction::BrIf(*label),
            ir::Instruction::Return => Instruction::Return,
            ir::Instruction::Call(func_idx) => Instruction::Call(*func_idx),
            ir::Instruction::End => Instruction::End,
        }
    }
}

pub fn run(binary: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = wasmtime::Config::new();
    config.wasm_gc(true);
    config.wasm_function_references(true);
    let engine = wasmtime::Engine::new(&config)?;
    let module = wasmtime::Module::from_binary(&engine, binary)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let mut linker = wasmtime::Linker::new(&engine);

    linker.func_wrap(
        "js",
        "print_int",
        |_caller: wasmtime::Caller<'_, ()>, i: i64| {
            println!("{i}");
        },
    )?;

    linker.func_wrap(
        "js",
        "print_float",
        |_caller: wasmtime::Caller<'_, ()>, i: f64| {
            println!("{i}");
        },
    )?;

    linker.func_wrap(
        "js",
        "print_bool",
        |_caller: wasmtime::Caller<'_, ()>, i: i32| {
            let i = i != 0;
            println!("{i}");
        },
    )?;

    let instance = linker.instantiate(&mut store, &module)?;
    let f = instance.get_typed_func::<(), ()>(&mut store, "main")?;
    f.call(&mut store, ())?;
    Ok(())
}

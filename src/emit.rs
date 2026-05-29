use crate::ir;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, Module,
    TypeSection, ValType,
};

pub fn emit(module: ir::Module) -> Vec<u8> {
    let mut bin_module = Module::new();

    let mut types = TypeSection::new();
    for ty in &module.types {
        match ty {
            ir::Type::Function(f) => {
                types.ty().function(
                    f.params.iter().map(ir::ValType::encode),
                    f.results.iter().map(ir::ValType::encode),
                );
            }
        }
    }
    bin_module.section(&types);

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

impl ir::ValType {
    fn encode(&self) -> ValType {
        match self {
            ir::ValType::I64 => ValType::I64,
            ir::ValType::F64 => ValType::F64,
            ir::ValType::I32 => ValType::I32,
        }
    }
}

impl ir::Instruction {
    fn encode(&self) -> Instruction<'_> {
        match self {
            ir::Instruction::LocalGet(idx) => Instruction::LocalGet(*idx),
            ir::Instruction::LocalSet(idx) => Instruction::LocalSet(*idx),
            ir::Instruction::I32Const(v) => Instruction::I32Const(*v),

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

            ir::Instruction::Call(func_idx) => Instruction::Call(*func_idx),
            ir::Instruction::End => Instruction::End,
        }
    }
}

pub fn run(binary: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_binary(&engine, binary)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let linker = wasmtime::Linker::new(&engine);
    let instance = linker.instantiate(&mut store, &module)?;
    let f = instance.get_typed_func::<(i64, i64), i64>(&mut store, "f")?;
    let result = f.call(&mut store, (10, 20))?;
    println!("The result of 10 + 20 is: {}", result);
    Ok(())
}

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Module, TypeSection, ValType,
};

pub fn emit() -> Vec<u8> {
    let mut module = Module::new();

    // Encode the type section.
    let mut types = TypeSection::new();
    let params = vec![ValType::I32, ValType::I32];
    let results = vec![ValType::I32];
    types.ty().function(params, results);
    module.section(&types);

    // Encode the function section.
    let mut functions = FunctionSection::new();
    let type_index = 0;
    functions.function(type_index);
    module.section(&functions);

    // Encode the export section.
    let mut exports = ExportSection::new();
    exports.export("f", ExportKind::Func, 0);
    module.section(&exports);

    // Encode the code section.
    let mut codes = CodeSection::new();
    let locals = vec![];
    let mut f = Function::new(locals);
    f.instructions().local_get(0).local_get(1).i32_add().end();
    codes.function(&f);
    module.section(&codes);

    // Extract the encoded Wasm bytes for this module.
    module.finish()
}

pub fn run(binary: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_binary(&engine, binary)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let linker = wasmtime::Linker::new(&engine);
    let instance = linker.instantiate(&mut store, &module)?;
    let f = instance.get_typed_func::<(i32, i32), i32>(&mut store, "f")?;
    let result = f.call(&mut store, (10, 20))?;
    println!("The result of 10 + 20 is: {}", result);
    Ok(())
}

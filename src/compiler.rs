#[cfg(test)]
mod test;

use crate::semantic::{Declaration, DeclarationId, DeclarationKind, Semantics, StructField, Type};
use crate::source::Symbol;
use crate::{ast, ir};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

struct Compiler {
    semantics: Rc<Semantics>,

    local_addresses: HashMap<DeclarationId, LocalAddress>, // TODO: Unify both addresses?
    func_addresses: HashMap<DeclarationId, FuncAddress>,
    loops: Loops,
    wasm_types: WasmTypes,

    /// Index of the current function's i64 scratch local, allocated after
    /// params and user locals on first use (see `scratch_i64`).
    scratch_i64: Option<ir::LocalIdx>,
    scratch_base: ir::LocalIdx,
}

#[derive(Clone, Copy)]
struct FuncAddress {
    idx: ir::FuncIdx,
    ty_idx: ir::TypeIdx,
}

#[derive(Clone, Copy)]
struct LocalAddress {
    idx: ir::LocalIdx,
    ty: ir::ValType,
}

/// Keeps track of control flow instructions
#[derive(Default)]
struct Loops {
    /// Number of structured control frames (`block`/`loop`/`if`) currently open.
    depth: u32,
    stack: VecDeque<LoopFrame>,
}

struct LoopFrame {
    break_target: u32,
    continue_target: u32,
}

impl Compiler {
    fn module(&mut self, module: &ast::Module) -> ir::Module {
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut exports = Vec::new();

        for declaration in &self.semantics.declarations {
            if let DeclarationKind::Import = &declaration.kind {
                let func_idx = self.func_addresses.len() as ir::FuncIdx;
                let ty_idx = self.wasm_types.import_type_idx(&declaration.ty);
                self.func_addresses.insert(
                    declaration.id,
                    FuncAddress {
                        idx: func_idx,
                        ty_idx,
                    },
                );
                let import = ir::Import {
                    module: "js".to_string(),
                    name: declaration.name.clone(),
                    func_type: ty_idx,
                };
                imports.push(import);
            }
        }

        let mut local_indices: HashMap<DeclarationId, ir::LocalIdx> = HashMap::new();
        for declaration in &self.semantics.declarations {
            match &declaration.kind {
                DeclarationKind::Function => {
                    let idx = self.func_addresses.len() as ir::FuncIdx;
                    let ty_idx = self.wasm_types.type_idx(&declaration.ty);
                    self.func_addresses
                        .insert(declaration.id, FuncAddress { idx, ty_idx });
                }
                DeclarationKind::Local { func_id } => {
                    let ty = self.wasm_types.val_ty(&declaration.ty);
                    let idx = *local_indices
                        .entry(*func_id)
                        .and_modify(|count| *count += 1)
                        .or_insert(0);
                    self.local_addresses
                        .insert(declaration.id, LocalAddress { idx, ty });
                }
                _ => {}
            }
        }

        for item in &module.items {
            if let ast::ItemKind::Function {
                export,
                name,
                params,
                body,
                ..
            } = &item.kind
            {
                let decl_id = self.declaration_id(name);
                let addr = *self
                    .func_addresses
                    .get(&decl_id)
                    .expect("function declaration not found");
                let f = self.function(addr.ty_idx, name, params, body);
                functions.push(f);
                if *export {
                    let export = self.func_export(addr.idx, name);
                    exports.push(export);
                }
            }
        }
        ir::Module {
            types: self.wasm_types.groups.clone(),
            imports,
            functions,
            exports,
        }
    }

    fn function(
        &mut self,
        ty: ir::TypeIdx,
        name: &ast::Identifier,
        params: &[ast::Param],
        body: &ast::Block,
    ) -> ir::Function {
        let func_id = self.declaration_id(name);
        let mut locals = self
            .function_locals(func_id)
            .skip(params.len())
            .map(|d| self.local_addresses[&d.id].ty)
            .collect::<Vec<ir::ValType>>();
        // Each function body starts with a clean control-frame state and no
        // scratch local; the scratch slot comes after params and user locals.
        // TODO: maybe add some sort of reset-frame or frame struct.
        self.loops.reset();
        self.scratch_base = (params.len() + locals.len()) as ir::LocalIdx;
        self.scratch_i64 = None;
        let mut ins = Vec::new();
        self.block(&mut ins, body);
        if self.scratch_i64.is_some() {
            locals.push(ir::ValType::I64);
        }
        ins.push(ir::Instruction::End);
        ir::Function {
            ty,
            locals,
            body: ins,
        }
    }

    fn block(&mut self, ins: &mut Vec<ir::Instruction>, block: &ast::Block) {
        match &block.kind {
            ast::BlockKind::Braces { statements } => {
                for stmt in statements {
                    self.stmt(ins, stmt);
                }
            }
            ast::BlockKind::Expr { expr } => {
                self.expr(ins, expr);
            }
        }
    }

    fn stmt(&mut self, ins: &mut Vec<ir::Instruction>, stmt: &ast::Stmt) {
        match &stmt.kind {
            ast::StmtKind::ExprStmt { expr } => {
                self.expr_stmt(ins, expr);
            }
            ast::StmtKind::Declaration {
                name, initializer, ..
            } => {
                if let Some(value) = initializer.as_ref() {
                    self.expr(ins, value);
                    let address = self.local_addr(name);
                    ins.push(ir::Instruction::LocalSet(address.idx));
                }
            }
            ast::StmtKind::While { condition, body } => {
                // Standard Wasm while loop:
                //
                //   block                ;; break target (label 1 from inside loop)
                //     loop               ;; continue target (label 0)
                //       <condition>
                //       i32.eqz
                //       br_if 1          ;; condition false -> exit the loop
                //       <body>
                //       br 0             ;; jump back to re-test the condition
                //     end
                //   end
                ins.push(ir::Instruction::Block(ir::BlockType::Empty));
                ins.push(ir::Instruction::Loop(ir::BlockType::Empty));
                self.loops.enter_loop();
                self.expr(ins, condition);
                ins.push(ir::Instruction::I32Eqz);
                ins.push(ir::Instruction::BrIf(1));
                match &body.kind {
                    ast::BlockKind::Braces { statements } => {
                        for stmt in statements {
                            self.stmt(ins, stmt);
                        }
                    }
                    ast::BlockKind::Expr { expr } => {
                        self.expr_stmt(ins, expr);
                    }
                }
                ins.push(ir::Instruction::Br(0));
                self.loops.exit_loop();
                ins.push(ir::Instruction::End);
                ins.push(ir::Instruction::End);
            }
        }
    }

    fn expr_stmt(&mut self, ins: &mut Vec<ir::Instruction>, expr: &ast::Expr) {
        self.expr(ins, expr);
        // Only "real" values (Int/Float/Bool) leave something on the stack to
        // drop. `Unit` produces nothing, and `Never` (return/break/continue)
        // diverges, so neither needs a `drop`.
        let ty = self.node_type(expr.node);
        if !ty.is_unit() && !ty.is_never() {
            ins.push(ir::Instruction::Drop);
        }
    }

    fn expr(&mut self, ins: &mut Vec<ir::Instruction>, expr: &ast::Expr) {
        match &expr.kind {
            ast::ExprKind::Literal { kind } => match kind {
                // An integer literal may have been typed as `Byte` from its
                // expected type; `Byte` values are i32 at runtime.
                ast::LiteralKind::Int(value) => {
                    if self.node_type(expr.node).is_byte() {
                        ins.push(ir::Instruction::I32Const(*value as i32))
                    } else {
                        ins.push(ir::Instruction::I64Const(*value))
                    }
                }
                ast::LiteralKind::Float(value) => ins.push(ir::Instruction::F64Const(*value)),
                ast::LiteralKind::Bool(value) => ins.push(ir::Instruction::I32Const(*value as i32)),
                ast::LiteralKind::Str(value) => {
                    let ty = self.node_type(expr.node);
                    let type_idx = self.wasm_types.type_idx(&ty);
                    // One i32.const per UTF-8 byte; semantic analysis caps
                    // literals at 10 000 bytes (the array.new_fixed limit).
                    for byte in value.bytes() {
                        ins.push(ir::Instruction::I32Const(byte as i32));
                    }
                    ins.push(ir::Instruction::ArrayNewFixed {
                        type_idx,
                        len: value.len() as u32,
                    });
                }
            },
            ast::ExprKind::Variable { name } => {
                let address = self.local_addr(name);
                ins.push(ir::Instruction::LocalGet(address.idx))
            }
            ast::ExprKind::Collection { elements } => {
                let ty = self.node_type(expr.node);
                let type_idx = self.wasm_types.type_idx(&ty);

                for element in elements {
                    self.expr(ins, element);
                }
                ins.push(ir::Instruction::ArrayNewFixed {
                    type_idx,
                    len: elements.len() as u32,
                });
            }
            ast::ExprKind::Mapping { fields } => {
                let ty = self.node_type(expr.node);
                let type_idx = self.wasm_types.ref_type_idx(&ty);
                let ty_fields = self.wasm_types.struct_fields(&ty).to_vec(); // TODO: optimize
                for ty_field in ty_fields {
                    // TODO: Consider using indexmap for fields
                    let field = fields
                        .iter()
                        .find(|&field| field.key.symbol == ty_field.name)
                        .expect("field not found");
                    self.expr(ins, &field.value);
                }

                ins.push(ir::Instruction::StructNew(type_idx));
            }
            ast::ExprKind::Unary { op, expr } => {
                let ty = self.node_type(expr.node);
                match (&op.kind, ty) {
                    (ast::UnOpKind::Neg, Type::Int) => {
                        ins.push(ir::Instruction::I64Const(0));
                        self.expr(ins, expr);
                        ins.push(ir::Instruction::I64Sub);
                    }
                    (ast::UnOpKind::Neg, Type::Float) => {
                        self.expr(ins, expr);
                        ins.push(ir::Instruction::F64Neg);
                    }
                    (ast::UnOpKind::Not, Type::Bool) => {
                        self.expr(ins, expr);
                        ins.push(ir::Instruction::I32Eqz);
                    }
                    _ => panic!("Unsupported unary operation"),
                }
            }
            ast::ExprKind::Binary { op, left, right } => {
                match &op.kind {
                    ast::BinOpKind::And => {
                        self.expr(ins, left);
                        ins.push(ir::Instruction::If(ir::BlockType::Result(ir::ValType::I32)));
                        self.loops.enter_frame();
                        self.expr(ins, right);
                        ins.push(ir::Instruction::Else);
                        ins.push(ir::Instruction::I32Const(0));
                        ins.push(ir::Instruction::End);
                        self.loops.exit_frame();
                        return;
                    }
                    ast::BinOpKind::Or => {
                        self.expr(ins, left);
                        ins.push(ir::Instruction::If(ir::BlockType::Result(ir::ValType::I32)));
                        self.loops.enter_frame();
                        ins.push(ir::Instruction::I32Const(1));
                        ins.push(ir::Instruction::Else);
                        self.expr(ins, right);
                        ins.push(ir::Instruction::End);
                        self.loops.exit_frame();
                        return;
                    }
                    _ => {}
                };
                self.expr(ins, left);
                self.expr(ins, right);
                let ty = self.node_type(left.node);
                match (&op.kind, ty) {
                    (ast::BinOpKind::Add, Type::Int) => ins.push(ir::Instruction::I64Add),
                    (ast::BinOpKind::Sub, Type::Int) => ins.push(ir::Instruction::I64Sub),
                    (ast::BinOpKind::Mul, Type::Int) => ins.push(ir::Instruction::I64Mul),
                    (ast::BinOpKind::Div, Type::Int) => ins.push(ir::Instruction::I64DivS),
                    (ast::BinOpKind::Mod, Type::Int) => ins.push(ir::Instruction::I64RemS),

                    (ast::BinOpKind::Add, Type::Float) => ins.push(ir::Instruction::F64Add),
                    (ast::BinOpKind::Sub, Type::Float) => ins.push(ir::Instruction::F64Sub),
                    (ast::BinOpKind::Mul, Type::Float) => ins.push(ir::Instruction::F64Mul),
                    (ast::BinOpKind::Div, Type::Float) => ins.push(ir::Instruction::F64Div),

                    (ast::BinOpKind::Eq, Type::Int) => ins.push(ir::Instruction::I64Eq),
                    (ast::BinOpKind::Ne, Type::Int) => ins.push(ir::Instruction::I64Ne),
                    (ast::BinOpKind::Ge, Type::Int) => ins.push(ir::Instruction::I64GeS),
                    (ast::BinOpKind::Gt, Type::Int) => ins.push(ir::Instruction::I64GtS),
                    (ast::BinOpKind::Le, Type::Int) => ins.push(ir::Instruction::I64LeS),
                    (ast::BinOpKind::Lt, Type::Int) => ins.push(ir::Instruction::I64LtS),

                    (ast::BinOpKind::Eq, Type::Bool) => ins.push(ir::Instruction::I32Eq),
                    (ast::BinOpKind::Ne, Type::Bool) => ins.push(ir::Instruction::I32Ne),

                    // `Byte` is i32 at runtime and unsigned, so comparisons
                    // use the unsigned i32 instructions.
                    (ast::BinOpKind::Eq, Type::Byte) => ins.push(ir::Instruction::I32Eq),
                    (ast::BinOpKind::Ne, Type::Byte) => ins.push(ir::Instruction::I32Ne),
                    (ast::BinOpKind::Ge, Type::Byte) => ins.push(ir::Instruction::I32GeU),
                    (ast::BinOpKind::Gt, Type::Byte) => ins.push(ir::Instruction::I32GtU),
                    (ast::BinOpKind::Le, Type::Byte) => ins.push(ir::Instruction::I32LeU),
                    (ast::BinOpKind::Lt, Type::Byte) => ins.push(ir::Instruction::I32LtU),

                    (ast::BinOpKind::Eq, Type::Float) => ins.push(ir::Instruction::F64Eq),
                    (ast::BinOpKind::Ne, Type::Float) => ins.push(ir::Instruction::F64Ne),
                    (ast::BinOpKind::Ge, Type::Float) => ins.push(ir::Instruction::F64Ge),
                    (ast::BinOpKind::Gt, Type::Float) => ins.push(ir::Instruction::F64Gt),
                    (ast::BinOpKind::Le, Type::Float) => ins.push(ir::Instruction::F64Le),
                    (ast::BinOpKind::Lt, Type::Float) => ins.push(ir::Instruction::F64Lt),

                    _ => panic!("Unsupported binary operation"),
                }
            }
            ast::ExprKind::Call { callee, args } => {
                let ast::ExprKind::Variable { name } = &callee.kind else {
                    panic!("Invalid callee");
                };
                for arg in args {
                    self.expr(ins, arg);
                }
                let decl_id = self.declaration_id(name);
                if let DeclarationKind::Builtin = self.semantics.declarations[decl_id].kind {
                    let builtin = self.semantics.declarations[decl_id].name.clone();
                    self.builtin_call(ins, &builtin);
                } else {
                    let address = self.func_addr(name);
                    ins.push(ir::Instruction::Call(address.idx));
                }
            }
            ast::ExprKind::Assignment { target, value } => match &target.kind {
                ast::ExprKind::Variable { name } => {
                    let address = self.local_addr(name);
                    self.expr(ins, value);
                    ins.push(ir::Instruction::LocalSet(address.idx));
                }
                ast::ExprKind::Index { expr, index } => {
                    let ty = self.node_type(expr.node);
                    let ty_idx = self.wasm_types.type_idx(&ty);
                    self.expr(ins, expr);
                    self.expr(ins, index);
                    ins.push(ir::Instruction::I32WrapI64);
                    self.expr(ins, value);
                    ins.push(ir::Instruction::ArraySet(ty_idx));
                }
                ast::ExprKind::Field { expr, field } => {
                    let ty = self.node_type(expr.node);
                    let type_idx = self.wasm_types.ref_type_idx(&ty);
                    let field_idx = self.wasm_types.struct_field_idx(&ty, &field.symbol);
                    self.expr(ins, expr);
                    self.expr(ins, value);
                    ins.push(ir::Instruction::StructSet {
                        type_idx,
                        field_idx,
                    });
                }
                _ => panic!("Invalid assignment target"),
            },
            ast::ExprKind::Index { expr, index } => {
                let ty = self.node_type(expr.node);
                let ty_idx = self.wasm_types.type_idx(&ty);
                self.expr(ins, expr);
                self.expr(ins, index);
                ins.push(ir::Instruction::I32WrapI64);
                // Packed arrays have no plain `array.get`: reads must pick a
                // sign extension, and `Byte` is unsigned.
                if is_byte_array(&ty) {
                    ins.push(ir::Instruction::ArrayGetU(ty_idx));
                } else {
                    ins.push(ir::Instruction::ArrayGet(ty_idx));
                }
            }
            ast::ExprKind::Field { expr, field } => {
                let ty = self.node_type(expr.node);
                let type_idx = self.wasm_types.ref_type_idx(&ty);
                let field_idx = self.wasm_types.struct_field_idx(&ty, &field.symbol);
                self.expr(ins, expr);
                ins.push(ir::Instruction::StructGet {
                    type_idx,
                    field_idx,
                });
            }
            ast::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expr(ins, condition);
                let ty = self.node_type(expr.node);
                // `Never` (e.g. both branches `return`) yields no value, so the
                // Wasm `if` has no result type, like `Unit`.
                let block_type = if ty.is_unit() || ty.is_never() {
                    ir::BlockType::Empty
                } else {
                    ir::BlockType::Result(self.wasm_types.val_ty(&ty))
                };
                ins.push(ir::Instruction::If(block_type));
                self.loops.enter_frame();
                self.branch(ins, then_branch);
                if let Some(else_branch) = else_branch {
                    ins.push(ir::Instruction::Else);
                    self.branch(ins, else_branch);
                } else {
                    // TODO: re-think when to drop and when not to drop.
                    let then_ty = self.node_type(then_branch.node);
                    if !then_ty.is_unit() && !then_ty.is_never() {
                        ins.push(ir::Instruction::Drop);
                    }
                }
                ins.push(ir::Instruction::End);
                self.loops.exit_frame();
            }
            ast::ExprKind::Break => {
                ins.push(ir::Instruction::Br(self.loops.break_label()));
            }
            ast::ExprKind::Continue => {
                ins.push(ir::Instruction::Br(self.loops.continue_label()));
            }
            ast::ExprKind::Return { expr: inner } => {
                // Evaluate the return value, then return from the function.
                self.expr(ins, inner);
                ins.push(ir::Instruction::Return);
            }
        }
    }

    /// Lowers a call to a builtin function to its inline instruction
    /// sequence. The arguments are already on the stack.
    fn builtin_call(&mut self, ins: &mut Vec<ir::Instruction>, name: &str) {
        use ir::Instruction as I;
        match name {
            "byte_to_int" => ins.push(I::I64ExtendI32U),
            "int_to_byte" => {
                // Traps unless 0 <= n <= 255; the unsigned comparison makes
                // negative values look huge.
                let tmp = self.scratch_i64();
                ins.push(I::LocalSet(tmp));
                ins.push(I::LocalGet(tmp));
                ins.push(I::I64Const(255));
                ins.push(I::I64GtU);
                ins.push(I::If(ir::BlockType::Empty));
                self.loops.enter_frame();
                ins.push(I::Unreachable);
                ins.push(I::End);
                self.loops.exit_frame();
                ins.push(I::LocalGet(tmp));
                ins.push(I::I32WrapI64);
            }
            "int_and" => ins.push(I::I64And),
            "int_or" => ins.push(I::I64Or),
            "int_xor" => ins.push(I::I64Xor),
            "int_shl" => ins.push(I::I64Shl),
            "int_shr" => ins.push(I::I64ShrS),
            "int_shr_u" => ins.push(I::I64ShrU),
            "int_rotl" => ins.push(I::I64Rotl),
            "int_rotr" => ins.push(I::I64Rotr),
            "int_clz" => ins.push(I::I64Clz),
            "int_ctz" => ins.push(I::I64Ctz),
            "int_popcnt" => ins.push(I::I64Popcnt),
            "int_div_u" => ins.push(I::I64DivU),
            "int_rem_u" => ins.push(I::I64RemU),
            "float_sqrt" => ins.push(I::F64Sqrt),
            "float_abs" => ins.push(I::F64Abs),
            "float_ceil" => ins.push(I::F64Ceil),
            "float_floor" => ins.push(I::F64Floor),
            "float_trunc" => ins.push(I::F64Trunc),
            "float_nearest" => ins.push(I::F64Nearest),
            "float_min" => ins.push(I::F64Min),
            "float_max" => ins.push(I::F64Max),
            "float_copysign" => ins.push(I::F64Copysign),
            "int_to_float" => ins.push(I::F64ConvertI64S),
            "float_to_int" => ins.push(I::I64TruncF64S),
            _ => panic!("Unknown builtin function '{name}'"),
        }
    }

    /// The current function's i64 scratch local, for inline builtin
    /// sequences. A single slot is safe to share: every sequence stores and
    /// consumes it without evaluating other expressions in between.
    fn scratch_i64(&mut self) -> ir::LocalIdx {
        *self.scratch_i64.get_or_insert(self.scratch_base)
    }

    // TODO: very similar to `block`, duplication
    fn branch(&mut self, ins: &mut Vec<ir::Instruction>, block: &ast::Block) {
        match &block.kind {
            ast::BlockKind::Braces { statements } => {
                if let Some((last, rest)) = statements.split_last() {
                    for stmt in rest {
                        self.stmt(ins, stmt);
                    }
                    // If the last statement is an expression statement, we want to simply
                    // evaluate it, normal `stmt` will drop the value from the stack.
                    if let ast::StmtKind::ExprStmt { expr } = &last.kind {
                        self.expr(ins, expr);
                    } else {
                        self.stmt(ins, last);
                    }
                }
            }
            ast::BlockKind::Expr { expr } => self.expr(ins, expr),
        }
    }

    fn func_export(&self, func_idx: ir::FuncIdx, name: &ast::Identifier) -> ir::Export {
        ir::Export {
            name: name.symbol.to_string(),
            kind: ir::ExportKind::Function(func_idx),
        }
    }

    fn local_addr(&self, identifier: &ast::Identifier) -> LocalAddress {
        let decl_id = self
            .semantics
            .uses
            .get(&identifier.node.id)
            .copied()
            .expect("Undeclared identifier");
        self.local_addresses
            .get(&decl_id)
            .copied()
            .expect("Address not found for declaration")
    }

    fn func_addr(&self, identifier: &ast::Identifier) -> FuncAddress {
        let decl_id = self
            .semantics
            .uses
            .get(&identifier.node.id)
            .copied()
            .expect("Undeclared function");
        self.func_addresses
            .get(&decl_id)
            .copied()
            .expect("Address not found for declaration")
    }

    fn declaration_id(&self, ident: &ast::Identifier) -> DeclarationId {
        self.semantics
            .uses
            .get(&ident.node.id)
            .copied()
            .expect("Declaration not found")
    }

    fn node_type(&self, node: ast::Node) -> Type {
        self.semantics
            .expr_types
            .get(&node.id)
            .cloned()
            .expect("Unknown type of node")
    }

    fn function_locals(&self, func_id: DeclarationId) -> impl Iterator<Item = &Declaration> {
        self.semantics.declarations.iter().filter(move |&d| {
            if let DeclarationKind::Local { func_id: decl_id } = d.kind {
                decl_id == func_id
            } else {
                false
            }
        })
    }
}

// TODO: make method
fn is_byte_array(ty: &Type) -> bool {
    matches!(ty, Type::Array { ty } if ty.is_byte())
}

struct WasmTypes {
    semantics: Rc<Semantics>,
    unique_types: HashMap<Type, ir::TypeIdx>,
    import_types: HashMap<Type, ir::TypeIdx>,
    groups: Vec<ir::RecGroup>,
    next_idx: ir::TypeIdx,
}

impl WasmTypes {
    fn new(semantics: Rc<Semantics>) -> Self {
        Self {
            semantics,
            unique_types: HashMap::new(),
            import_types: HashMap::new(),
            groups: vec![],
            next_idx: 0,
        }
    }

    /// Returns the Wasm type index for `ty`, defining it (and every type it
    /// depends on) if it is not defined yet.
    ///
    /// New types are defined per strongly-connected component of the type
    /// dependency graph: each SCC becomes one recursion group, appended after
    /// the groups it depends on. Only types that are mutually recursive end
    /// up sharing a group; everything else gets a single-type group.
    fn type_idx(&mut self, ty: &Type) -> ir::TypeIdx {
        let ty = self.canonical(ty);
        if let Some(idx) = self.unique_types.get(&ty) {
            return *idx;
        }

        let sccs = {
            let mut tarjan = Tarjan::new(self);
            tarjan.visit(&ty);
            tarjan.sccs
        };
        for scc in sccs {
            // Reserve indices for the whole group up front so that mutually
            // recursive members can refer to each other while being lowered.
            for member in &scc {
                let idx = self.next_idx;
                self.next_idx += 1;
                self.unique_types.insert(member.clone(), idx);
            }
            let types = scc.iter().map(|member| self.lower(member)).collect();
            self.groups.push(ir::RecGroup { types });
        }

        self.unique_types[&ty]
    }

    /// Returns the Wasm type index for an imported (host) function's
    /// signature. `String` params and returns are lowered to the abstract
    /// `arrayref` type instead of the concrete byte-array type: a typed host
    /// function can only be declared over abstract GC types, and Wasm
    /// function-type subtyping is nominal, so the import's declared type must
    /// match the host's exactly. Call sites stay valid because a concrete
    /// array is a subtype of `arrayref`.
    ///
    /// Signatures without `String` share the ordinary `type_idx` entry.
    fn import_type_idx(&mut self, ty: &Type) -> ir::TypeIdx {
        let Type::Function { params, ret } = ty else {
            panic!("Import type is not a function");
        };
        let involves_string = params
            .iter()
            .chain(std::iter::once(ret.as_ref()))
            .any(|t| matches!(t, Type::String));
        if !involves_string {
            return self.type_idx(ty);
        }
        if let Some(idx) = self.import_types.get(ty) {
            return *idx;
        }

        let params = params
            .iter()
            .map(|p| match p {
                Type::String => ir::ValType::ArrayRef,
                other => self.val_ty(other),
            })
            .collect();
        let results = match ret.as_ref() {
            Type::Unit => vec![],
            Type::String => vec![ir::ValType::ArrayRef],
            other => vec![self.val_ty(other)],
        };
        let idx = self.next_idx;
        self.next_idx += 1;
        self.groups.push(ir::RecGroup {
            types: vec![ir::Type::Function { params, results }],
        });
        self.import_types.insert(ty.clone(), idx);
        idx
    }

    /// Lowers a canonical heap type to its Wasm definition. Every heap type
    /// it refers to must already have an index in `unique_types`.
    fn lower(&mut self, ty: &Type) -> ir::Type {
        match ty {
            Type::Function { params, ret } => {
                let params = params.iter().map(|p| self.val_ty(p)).collect::<Vec<_>>();
                let results = if let Type::Unit = ret.as_ref() {
                    vec![]
                } else {
                    vec![self.val_ty(ret)]
                };
                ir::Type::Function { params, results }
            }
            Type::Array { ty } => ir::Type::Array {
                ty: if ty.is_byte() {
                    ir::StorageType::I8
                } else {
                    ir::StorageType::Val(self.val_ty(ty))
                },
            },
            Type::Struct { fields, .. } => ir::Type::Struct {
                fields: fields
                    .iter()
                    .map(|f| ir::StorageType::Val(self.val_ty(&f.ty)))
                    .collect(),
            },
            _ => panic!("Type is not part of types section"),
        }
    }

    /// The canonical form of a type used as key in `unique_types`: struct
    /// references are resolved to the struct type they name.
    fn canonical(&self, ty: &Type) -> Type {
        self.heap_type(ty).unwrap_or_else(|| ty.clone())
    }

    /// Returns the canonical heap type a value of `ty` points to, if `ty` is
    /// represented as a Wasm reference.
    fn heap_type(&self, ty: &Type) -> Option<Type> {
        match ty {
            Type::Array { .. } => Some(ty.clone()),
            Type::String => Some(Type::Array {
                ty: Rc::new(Type::Byte),
            }),
            Type::Reference { name } => {
                let struct_ty = self
                    .semantics
                    .struct_types
                    .get(name)
                    .cloned()
                    .expect("struct type not found");
                Some(struct_ty)
            }
            _ => None,
        }
    }

    /// The canonical heap types that `ty`'s Wasm definition refers to, in
    /// definition order. These are the edges of the type dependency graph.
    fn heap_deps(&self, ty: &Type) -> Vec<Type> {
        match ty {
            Type::Function { params, ret } => params
                .iter()
                .chain(std::iter::once(ret.as_ref()))
                .filter_map(|t| self.heap_type(t))
                .collect(),
            Type::Array { ty } => self.heap_type(ty).into_iter().collect(),
            Type::Struct { fields, .. } => fields
                .iter()
                .filter_map(|f| self.heap_type(&f.ty))
                .collect(),
            _ => vec![],
        }
    }

    fn val_ty(&mut self, ty: &Type) -> ir::ValType {
        match ty {
            Type::Int => ir::ValType::I64,
            Type::Float => ir::ValType::F64,
            Type::Bool => ir::ValType::I32,
            Type::Byte => ir::ValType::I32,
            Type::Array { .. } | Type::Reference { .. } | Type::String => {
                ir::ValType::Ref(self.type_idx(ty))
            }
            _ => panic!("Type does not have equivalent Wasm value type"),
        }
    }

    fn ref_type_idx(&mut self, ty: &Type) -> ir::TypeIdx {
        let ir::ValType::Ref(type_idx) = self.val_ty(ty) else {
            panic!("Type is not a reference type");
        };
        type_idx
    }

    fn struct_field_idx(&mut self, ty: &Type, field_name: &Symbol) -> ir::FieldIdx {
        let Type::Reference { name: ref_name } = ty else {
            panic!("Type is not a reference")
        };
        let Some(Type::Struct {
            name: struct_name,
            fields,
        }) = self.semantics.struct_types.get(ref_name)
        else {
            panic!("Struct type not found")
        };

        debug_assert!(struct_name == ref_name);

        let Some(idx) = fields.iter().position(|field| field.name == *field_name) else {
            panic!("Field not found")
        };
        idx as ir::FieldIdx
    }

    fn struct_fields(&self, ty: &Type) -> &[StructField] {
        let Type::Reference { name: ref_name } = ty else {
            panic!("Type is not a reference")
        };
        let Some(Type::Struct {
            name: struct_name,
            fields,
        }) = self.semantics.struct_types.get(ref_name)
        else {
            panic!("Struct type not found")
        };
        debug_assert!(struct_name == ref_name);
        fields
    }
}

/// Tarjan's strongly-connected-components algorithm over the type dependency
/// graph, restricted to types that don't have a Wasm index yet (types that
/// are already defined live in earlier groups and are always valid to
/// reference, so they are not part of the graph).
///
/// `sccs` ends up ordered dependencies-first: an SCC appears after every SCC
/// it depends on, which is exactly the order recursion groups must be defined
/// in the type section.
/// See: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
struct Tarjan<'a> {
    wasm_types: &'a WasmTypes,
    index: HashMap<Type, usize>,
    lowlink: HashMap<Type, usize>,
    stack: Vec<Type>,
    on_stack: HashSet<Type>,
    next_index: usize,
    sccs: Vec<Vec<Type>>,
}

impl<'a> Tarjan<'a> {
    fn new(wasm_types: &'a WasmTypes) -> Self {
        Self {
            wasm_types,
            index: HashMap::new(),
            lowlink: HashMap::new(),
            stack: Vec::new(),
            on_stack: HashSet::new(),
            next_index: 0,
            sccs: Vec::new(),
        }
    }

    fn visit(&mut self, ty: &Type) {
        let index = self.next_index;
        self.next_index += 1;
        self.index.insert(ty.clone(), index);
        self.lowlink.insert(ty.clone(), index);
        self.stack.push(ty.clone());
        self.on_stack.insert(ty.clone());

        for dep in self.wasm_types.heap_deps(ty) {
            if self.wasm_types.unique_types.contains_key(&dep) {
                continue;
            }
            match self.index.get(&dep) {
                None => {
                    self.visit(&dep);
                    let dep_lowlink = self.lowlink[&dep];
                    let lowlink = self.lowlink.get_mut(ty).unwrap();
                    *lowlink = (*lowlink).min(dep_lowlink);
                }
                Some(&dep_index) => {
                    if self.on_stack.contains(&dep) {
                        let lowlink = self.lowlink.get_mut(ty).unwrap();
                        *lowlink = (*lowlink).min(dep_index);
                    }
                }
            }
        }

        if self.lowlink[ty] == index {
            let mut scc = Vec::new();
            loop {
                let member = self.stack.pop().expect("SCC stack is empty");
                self.on_stack.remove(&member);
                let is_root = member == *ty;
                scc.push(member);
                if is_root {
                    break;
                }
            }
            // Members were popped in reverse visit order; restore it so the
            // group lists types in the order they were first reached.
            scc.reverse();
            self.sccs.push(scc);
        }
    }
}

impl Loops {
    fn reset(&mut self) {
        self.depth = 0;
        self.stack.clear();
    }

    fn enter_frame(&mut self) {
        self.depth += 1;
    }

    fn exit_frame(&mut self) {
        self.depth -= 1;
    }

    fn enter_loop(&mut self) {
        let base = self.depth;
        self.stack.push_back(LoopFrame {
            break_target: base,
            continue_target: base + 1,
        });
        self.depth += 2;
    }

    fn exit_loop(&mut self) {
        self.stack.pop_back();
        self.depth -= 2;
    }

    fn break_label(&self) -> ir::LabelIdx {
        let target = self
            .stack
            .back()
            .expect("`break` outside of a loop")
            .break_target;
        (self.depth - 1) - target
    }

    fn continue_label(&self) -> ir::LabelIdx {
        let target = self
            .stack
            .back()
            .expect("`continue` outside of a loop")
            .continue_target;
        (self.depth - 1) - target
    }
}

pub fn compile(module: &ast::Module, semantics: Semantics) -> ir::Module {
    let semantics = Rc::from(semantics);
    let mut compiler = Compiler {
        semantics: Rc::clone(&semantics),
        wasm_types: WasmTypes::new(Rc::clone(&semantics)),
        local_addresses: Default::default(),
        func_addresses: Default::default(),
        loops: Default::default(),
        scratch_i64: None,
        scratch_base: 0,
    };
    compiler.module(module)
}

#[cfg(test)]
mod test;

use crate::ast::ItemKind;
use crate::ir::StorageType;
use crate::semantic::{Declaration, DeclarationId, DeclarationKind, Semantics, Type};
use crate::{ast, ir};
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
struct Compiler {
    semantics: Semantics,

    local_addresses: HashMap<DeclarationId, LocalAddress>, // TODO: Unify both addresses?
    func_addresses: HashMap<DeclarationId, FuncAddress>,
    loops: Loops,
    wasm_types: Types,
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
                let ty_idx = self.wasm_types.get_or_create(&declaration.ty);
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
                    let ty_idx = self.wasm_types.get_or_create(&declaration.ty);
                    self.func_addresses
                        .insert(declaration.id, FuncAddress { idx, ty_idx });
                }
                DeclarationKind::Local(fn_id) => {
                    let ty = match &declaration.ty {
                        Type::Array { .. } => {
                            let ty_idx = self.wasm_types.get_or_create(&declaration.ty);
                            ir::ValType::Ref(ty_idx)
                        }
                        _ => declaration.ty.lower_to_val_type(),
                    };
                    let idx = *local_indices
                        .entry(*fn_id)
                        .and_modify(|count| *count += 1)
                        .or_insert(0);
                    self.local_addresses
                        .insert(declaration.id, LocalAddress { idx, ty });
                }
                _ => {}
            }
        }

        for item in &module.items {
            if let ItemKind::Function {
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
            types: self.wasm_types.types.clone(),
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
        let locals = self
            .function_locals(func_id)
            .skip(params.len())
            .map(|d| self.local_addresses[&d.id].ty)
            .collect::<Vec<ir::ValType>>();
        // Each function body starts with a clean control-frame state.
        self.loops.reset();
        let mut ins = Vec::new();
        self.block(&mut ins, body);
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
                ast::LiteralKind::Int(value) => ins.push(ir::Instruction::I64Const(*value)),
                ast::LiteralKind::Float(value) => ins.push(ir::Instruction::F64Const(*value)),
                ast::LiteralKind::Bool(value) => ins.push(ir::Instruction::I32Const(*value as i32)),
            },
            ast::ExprKind::Variable { name } => {
                let address = self.local_addr(name);
                ins.push(ir::Instruction::LocalGet(address.idx))
            }
            ast::ExprKind::Collection { elements } => {
                let ty = self.node_type(expr.node);
                let type_idx = self.wasm_types.get(&ty);

                for element in elements {
                    self.expr(ins, element);
                }
                ins.push(ir::Instruction::ArrayNewFixed {
                    type_idx,
                    len: elements.len() as u32,
                });
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
                let address = self.func_addr(name);
                for arg in args {
                    self.expr(ins, arg);
                }
                ins.push(ir::Instruction::Call(address.idx));
            }
            ast::ExprKind::Assignment { target, value } => {
                let ast::ExprKind::Variable { name } = &target.kind else {
                    panic!("Invalid assignment target");
                };
                let address = self.local_addr(name);
                self.expr(ins, value);
                ins.push(ir::Instruction::LocalSet(address.idx));
            }
            ast::ExprKind::Index { expr, index } => {
                let ty = self.node_type(expr.node);
                let ty = self.wasm_types.get(&ty);
                self.expr(ins, expr);
                self.expr(ins, index);
                ins.push(ir::Instruction::I32WrapI64);
                ins.push(ir::Instruction::ArrayGet(ty));
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
                    ir::BlockType::Result(ty.lower_to_val_type())
                };
                ins.push(ir::Instruction::If(block_type));
                self.loops.enter_frame();
                self.branch(ins, then_branch);
                if let Some(else_branch) = else_branch {
                    ins.push(ir::Instruction::Else);
                    self.branch(ins, else_branch);
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
            if let DeclarationKind::Local(decl_id) = d.kind {
                decl_id == func_id
            } else {
                false
            }
        })
    }
}

#[derive(Default)]
struct Types {
    unique_types: HashMap<Type, ir::TypeIdx>,
    types: Vec<ir::Type>,
}

impl Types {
    fn get_or_create(&mut self, ty: &Type) -> ir::TypeIdx {
        if let Some(idx) = self.unique_types.get(ty) {
            return *idx;
        }

        let idx = self.types.len() as ir::TypeIdx;
        self.types.push(ty.lower_to_type());
        self.unique_types.insert(ty.clone(), idx);
        idx
    }

    fn get(&self, ty: &Type) -> ir::TypeIdx {
        self.unique_types.get(ty).cloned().expect("Unknown type")
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

impl Type {
    fn lower_to_val_type(&self) -> ir::ValType {
        match self {
            Type::Int => ir::ValType::I64,
            Type::Float => ir::ValType::F64,
            Type::Bool => ir::ValType::I32,
            _ => todo!(),
        }
    }

    fn lower_to_type(&self) -> ir::Type {
        match self {
            Type::Function { params, ret } => {
                let params = params
                    .iter()
                    .map(|p| p.lower_to_val_type())
                    .collect::<Vec<_>>();
                let results = if let Type::Unit = ret.as_ref() {
                    vec![]
                } else {
                    vec![ret.lower_to_val_type()]
                };
                ir::Type::Function { params, results }
            }
            Type::Array { ty } => ir::Type::Array {
                ty: StorageType::Val(ty.lower_to_val_type()),
            },
            _ => panic!("Type is not part of types section"),
        }
    }
}

pub fn compile(module: &ast::Module, semantics: Semantics) -> ir::Module {
    let mut compiler = Compiler {
        semantics,
        ..Default::default()
    };
    compiler.module(module)
}

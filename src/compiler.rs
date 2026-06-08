#[cfg(test)]
mod test;

use crate::ast::ItemKind;
use crate::semantic::{Declaration, DeclarationId, DeclarationKind, Semantics, Type};
use crate::{ast, ir};
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
struct Compiler {
    local_addresses: HashMap<DeclarationId, ir::LocalIdx>,
    func_addresses: HashMap<DeclarationId, ir::FuncIdx>,
    semantics: Semantics,
    loops: Loops,
}

/// Tracks the open Wasm control frames while lowering a function body so that
/// `break`/`continue` can compute their relative branch label indices.
#[derive(Default)]
struct Loops {
    /// Number of structured control frames (`block`/`loop`/`if`) currently open.
    depth: u32,
    /// Enclosing loops. The back of the deque is the innermost loop — the target
    /// of `break`/`continue`.
    stack: VecDeque<LoopFrame>,
}

struct LoopFrame {
    break_target: u32,
    continue_target: u32,
}

impl Loops {
    fn reset(&mut self) {
        self.depth = 0;
        self.stack.clear();
    }

    /// Opens a non-loop control frame (an `if`, or an `and`/`or` short-circuit block).
    fn enter_frame(&mut self) {
        self.depth += 1;
    }

    fn exit_frame(&mut self) {
        self.depth -= 1;
    }

    /// Opens a loop's outer `block` and inner `loop` frames and records the loop
    /// as the current `break`/`continue` target.
    fn enter_loop(&mut self) {
        let base = self.depth;
        // `break` exits the outer `block` (frame `base`); `continue` jumps to the
        // inner `loop` (frame `base + 1`).
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

    /// Relative label index for `break` (branches out of the innermost loop's `block`).
    fn break_label(&self) -> ir::LabelIdx {
        let target = self
            .stack
            .back()
            .expect("`break` outside of a loop")
            .break_target;
        (self.depth - 1) - target
    }

    /// Relative label index for `continue` (branches back to the innermost `loop`).
    fn continue_label(&self) -> ir::LabelIdx {
        let target = self
            .stack
            .back()
            .expect("`continue` outside of a loop")
            .continue_target;
        (self.depth - 1) - target
    }
}

impl Compiler {
    fn module(&mut self, module: &ast::Module) -> ir::Module {
        let mut types = Vec::new();
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut exports = Vec::new();

        for item in &module.items {
            match &item.kind {
                ItemKind::Import { name, .. } => {
                    let decl_id = self.declaration_id(name);
                    let func_idx = self.func_addresses.len() as ir::FuncIdx;
                    self.func_addresses.insert(decl_id, func_idx);
                    let ty = self.func_type(name);
                    let ty_idx = types.len() as ir::TypeIdx;
                    types.push(ir::Type::Function(ty));
                    let import = ir::Import {
                        module: "js".to_string(),
                        name: name.symbol.clone(),
                        func_type: ty_idx,
                    };
                    imports.push(import);
                }
                ItemKind::Function { name, .. } => {
                    let decl_id = self.declaration_id(name);
                    let func_idx = self.func_addresses.len() as ir::FuncIdx;
                    self.func_addresses.insert(decl_id, func_idx);
                }
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
                let func_idx = *self
                    .func_addresses
                    .get(&decl_id)
                    .expect("function declaration not found");
                let ty = self.func_type(name);
                let ty_idx = types.len() as ir::TypeIdx;
                types.push(ir::Type::Function(ty));
                let f = self.function(ty_idx, name, params, body);
                functions.push(f);
                if *export {
                    let export = self.func_export(func_idx, name);
                    exports.push(export);
                }
            }
        }
        ir::Module {
            types,
            imports,
            functions,
            exports,
        }
    }

    fn func_type(&self, name: &ast::Identifier) -> ir::FuncType {
        let Type::Function { params, ret } = self.declaration_type(name) else {
            panic!("Function is not of type function")
        };

        let params = params.iter().map(|p| p.lower()).collect::<Vec<_>>();
        let results = if let Type::Unit = ret.as_ref() {
            vec![]
        } else {
            vec![ret.lower()]
        };
        ir::FuncType { params, results }
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
            .map(|d| d.ty.lower())
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
                name,
                initializer: value,
                ..
            } => {
                self.expr(ins, value);
                let address = self.local_addr(name);
                ins.push(ir::Instruction::LocalSet(address));
            }
            ast::StmtKind::Assignment { target, value } => {
                let ast::ExprKind::Variable { name } = &target.kind else {
                    panic!("Invalid assignment target");
                };
                let adddress = self.local_addr(name);
                self.expr(ins, value);
                ins.push(ir::Instruction::LocalSet(adddress));
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
            ast::StmtKind::Return { expr } => {
                self.expr(ins, expr);
            }
        }
    }

    fn expr_stmt(&mut self, ins: &mut Vec<ir::Instruction>, expr: &ast::Expr) {
        self.expr(ins, expr);
        if !self.node_type(expr.node).is_unit() {
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
                ins.push(ir::Instruction::LocalGet(address))
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
                ins.push(ir::Instruction::Call(address));
            }
            ast::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expr(ins, condition);
                let ty = self.node_type(expr.node);
                let block_type = if ty.is_unit() {
                    ir::BlockType::Empty
                } else {
                    ir::BlockType::Result(ty.lower())
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

    fn calculate_addresses(&mut self) {
        let mut local_indices: HashMap<DeclarationId, ir::LocalIdx> = HashMap::new();
        for decl in &self.semantics.declarations {
            if let DeclarationKind::Local(fn_id) = decl.kind {
                let address = *local_indices
                    .entry(fn_id)
                    .and_modify(|count| *count += 1)
                    .or_insert(0);
                self.local_addresses.insert(decl.id, address);
            };
        }
    }

    fn local_addr(&self, identifier: &ast::Identifier) -> ir::LocalIdx {
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

    // TODO: Unify
    fn func_addr(&self, identifier: &ast::Identifier) -> ir::FuncIdx {
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

    fn declaration_type(&self, ident: &ast::Identifier) -> Type {
        let decl_id = self.declaration_id(ident);
        let decl = &self.semantics.declarations[decl_id];
        decl.ty.clone()
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

impl Type {
    fn lower(&self) -> ir::ValType {
        match self {
            Type::Int => ir::ValType::I64,
            Type::Float => ir::ValType::F64,
            Type::Bool => ir::ValType::I32,
            _ => todo!(),
        }
    }
}

pub fn compile(module: &ast::Module, semantics: Semantics) -> ir::Module {
    let mut compiler = Compiler {
        semantics,
        ..Default::default()
    };
    compiler.calculate_addresses();
    compiler.module(module)
}

use crate::error::CompilerError;
use crate::semantic::{Declaration, DeclarationId, DeclarationKind, Semantics};
use crate::{ast, ir};
use std::collections::HashMap;

struct Compiler {
    local_addresses: HashMap<DeclarationId, ir::LocalIdx>,
    func_addresses: HashMap<DeclarationId, ir::FuncIdx>,
    semantics: Semantics,
}

pub type Result<T> = std::result::Result<T, CompilerError>;

impl Compiler {
    fn module(&self, module: &ast::Module) -> Result<ir::Module> {
        let mut types = Vec::new();
        let mut functions = Vec::new();
        let mut exports = Vec::new();
        for function in &module.items {
            let ty = self.func_type(function)?;
            let ty_idx = types.len() as ir::TypeIdx;
            types.push(ir::Type::Function(ty));
            let func_idx = functions.len() as ir::FuncIdx;
            let f = self.function(ty_idx, function)?;
            functions.push(f);
            let export = self.func_export(func_idx, function)?;
            exports.push(export);
        }
        Ok(ir::Module {
            types,
            functions,
            exports,
        })
    }

    fn func_type(&self, function: &ast::Function) -> Result<ir::FuncType> {
        let params = function
            .params
            .iter()
            .map(|param| param.ty.kind.lower())
            .collect::<Result<Vec<_>>>()?;
        let results = if let Some(ty) = &function.return_ty {
            vec![ty.kind.lower()?]
        } else {
            vec![]
        };
        Ok(ir::FuncType { params, results })
    }

    fn function(&self, ty: ir::TypeIdx, function: &ast::Function) -> Result<ir::Function> {
        let func_id = self.declaration_id(&function.name);
        let locals = self
            .function_locals(func_id)
            .skip(function.params.len())
            .map(|_d| ir::ValType::I64)
            .collect::<Vec<ir::ValType>>();
        let mut body = Vec::new();
        self.stmt(&mut body, &function.body)?;
        body.push(ir::Instruction::End);
        Ok(ir::Function { ty, locals, body })
    }

    fn stmt(&self, ins: &mut Vec<ir::Instruction>, stmt: &ast::Stmt) -> Result<()> {
        match &stmt.kind {
            ast::StmtKind::ExprStmt { expr } => self.expr(ins, expr)?,
            ast::StmtKind::Block { statements } => {
                for stmt in statements {
                    self.stmt(ins, stmt)?;
                }
            }
            ast::StmtKind::Declaration {
                name,
                initializer: value,
                ..
            } => {
                self.expr(ins, value)?;
                let address = self.local_addr(name);
                ins.push(ir::Instruction::LocalSet(address));
            }
            ast::StmtKind::Assignment { target, value } => {
                let ast::ExprKind::Variable { name } = &target.kind else {
                    panic!("Invalid assignment target");
                };
                let adddress = self.local_addr(name);
                self.expr(ins, value)?;
                ins.push(ir::Instruction::LocalSet(adddress));
            }
            ast::StmtKind::Return { expr } => {
                self.expr(ins, expr)?;
            }
        }
        Ok(())
    }

    fn expr(&self, ins: &mut Vec<ir::Instruction>, expr: &ast::Expr) -> Result<()> {
        match &expr.kind {
            ast::ExprKind::Literal { kind } => match kind {
                ast::LiteralKind::Int(value) => ins.push(ir::Instruction::I64Const(*value)),
                _ => todo!(),
            },
            ast::ExprKind::Variable { name } => {
                let address = self.local_addr(name);
                ins.push(ir::Instruction::LocalGet(address))
            }
            ast::ExprKind::Unary { op, expr } => match &op.kind {
                ast::UnOpKind::Neg => {
                    ins.push(ir::Instruction::I64Const(0));
                    self.expr(ins, expr)?;
                    ins.push(ir::Instruction::I64Sub);
                }
            },
            ast::ExprKind::Binary { op, left, right } => {
                self.expr(ins, left)?;
                self.expr(ins, right)?;
                match &op.kind {
                    ast::BinOpKind::Add => ins.push(ir::Instruction::I64Add),
                    ast::BinOpKind::Sub => ins.push(ir::Instruction::I64Sub),
                    ast::BinOpKind::Mul => ins.push(ir::Instruction::I64Mul),
                    ast::BinOpKind::Div => ins.push(ir::Instruction::I64DivS),
                    ast::BinOpKind::Mod => ins.push(ir::Instruction::I64RemS),
                    _ => todo!(),
                }
            }
            ast::ExprKind::Call { callee, args } => {
                let ast::ExprKind::Variable { name } = &callee.kind else {
                    panic!("Invalid callee");
                };
                let address = self.func_addr(name);
                for arg in args {
                    self.expr(ins, arg)?;
                }
                ins.push(ir::Instruction::Call(address));
            }
        }
        Ok(())
    }

    fn func_export(&self, func_idx: ir::FuncIdx, func: &ast::Function) -> Result<ir::Export> {
        Ok(ir::Export {
            name: func.name.symbol.to_string(),
            kind: ir::ExportKind::Function(func_idx),
        })
    }

    fn calculate_addresses(&mut self) {
        let mut local_indices: HashMap<DeclarationId, ir::LocalIdx> = HashMap::new();
        let mut func_address = 0;
        for decl in &self.semantics.declarations {
            match decl.kind {
                DeclarationKind::Local(fn_id) => {
                    let address = *local_indices
                        .entry(fn_id)
                        .and_modify(|count| *count += 1)
                        .or_insert(0);
                    self.local_addresses.insert(decl.id, address);
                }
                DeclarationKind::Function => {
                    self.func_addresses.insert(decl.id, func_address);
                    func_address += 1;
                }
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

impl ast::TypeKind {
    fn lower(&self) -> Result<ir::ValType> {
        match self {
            ast::TypeKind::Int => Ok(ir::ValType::I64),
            ast::TypeKind::Float => Ok(ir::ValType::F64),
            ast::TypeKind::Bool => todo!("Implement bool type"),
        }
    }
}

pub fn compile(module: &ast::Module, semantics: Semantics) -> Result<ir::Module> {
    let mut compiler = Compiler {
        semantics,
        local_addresses: Default::default(),
        func_addresses: Default::default(),
    };
    compiler.calculate_addresses();
    compiler.module(module)
}

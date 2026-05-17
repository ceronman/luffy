use crate::error::CompilerError;
use crate::ir::LocalIdx;
use crate::semantic::{DeclarationId, DeclarationKind};
use crate::{ast, ir, semantic};
use std::collections::HashMap;

struct Compiler {
    addresses: HashMap<DeclarationId, LocalIdx>,
    semantics: semantic::Semantics,
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
        let mut body = Vec::new();
        self.stmt(&mut body, &function.body)?;
        body.push(ir::Instruction::End);
        Ok(ir::Function { ty, body })
    }

    fn stmt(&self, ins: &mut Vec<ir::Instruction>, stmt: &ast::Stmt) -> Result<()> {
        match &stmt.kind {
            ast::StmtKind::ExprStmt { expr } => self.expr(ins, expr)?,
            ast::StmtKind::Block { statements } => {
                for stmt in statements {
                    self.stmt(ins, stmt)?;
                }
            }
            ast::StmtKind::Declaration { .. } => todo!(),
            ast::StmtKind::Assignment { .. } => todo!(),
            ast::StmtKind::Return { expr } => {
                self.expr(ins, expr)?;
            }
        }
        Ok(())
    }

    fn expr(&self, ins: &mut Vec<ir::Instruction>, expr: &ast::Expr) -> Result<()> {
        match &expr.kind {
            ast::ExprKind::Literal { .. } => todo!(),
            ast::ExprKind::Variable { name } => {
                let decl_id = self
                    .semantics
                    .uses
                    .get(&name.node.id)
                    .copied()
                    .expect("Undeclared identifier");
                let address = self
                    .addresses
                    .get(&decl_id)
                    .copied()
                    .expect("Address not found for declaration");
                ins.push(ir::Instruction::LocalGet(address))
            }
            ast::ExprKind::Unary { .. } => todo!(),
            ast::ExprKind::Binary { op, left, right } => match &op.kind {
                ast::BinOpKind::Add => {
                    self.expr(ins, left)?;
                    self.expr(ins, right)?;
                    ins.push(ir::Instruction::I64Add);
                }
                _ => todo!(),
            },
            ast::ExprKind::Call { .. } => todo!(),
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
        let mut local_indices: HashMap<DeclarationId, LocalIdx> = HashMap::new();
        for decl in &self.semantics.declarations {
            let address = match decl.kind {
                DeclarationKind::Local(fn_id) => *local_indices
                    .entry(fn_id)
                    .and_modify(|count| *count += 1)
                    .or_insert(0),
                _ => continue,
            };
            self.addresses.insert(decl.id, address);
        }
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

pub fn compile(module: &ast::Module, semantics: semantic::Semantics) -> Result<ir::Module> {
    let mut compiler = Compiler {
        semantics,
        addresses: Default::default(),
    };
    compiler.calculate_addresses();
    compiler.module(module)
}

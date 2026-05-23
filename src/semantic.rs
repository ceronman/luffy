use crate::ast::{
    Expr, ExprKind, Function, Identifier, LiteralKind, Module, NodeId, Param, Stmt, StmtKind,
    Symbol, TypeKind, TypeRef,
};
use crate::error::{CompilerError, ErrorKind};
use crate::lexer::Span;
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::rc::Rc;

pub type DeclarationId = usize;

pub struct Declaration {
    pub id: DeclarationId,
    pub ty: Type,
    pub kind: DeclarationKind,
}

pub enum DeclarationKind {
    Local(DeclarationId),
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Int,
    Float,
    Bool,
    Function { params: Rc<[Type]>, ret: Rc<Type> },
}

#[derive(Default)]
pub struct Semantics {
    pub expr_types: HashMap<NodeId, Type>,
    pub declarations: Vec<Declaration>,
    pub uses: HashMap<NodeId, DeclarationId>,
}

type Scope = HashMap<Symbol, DeclarationId>;

#[derive(Default)]
struct Resolver {
    scopes: VecDeque<Scope>,
    semantics: Semantics,
}

pub type Result<T> = std::result::Result<T, CompilerError>;

fn error<T: Debug>(span: Span, message: impl Into<String>) -> crate::parser::Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Resolve,
        msg: message.into(),
        span,
    })
}

impl Resolver {
    fn module(&mut self, module: &Module) -> Result<()> {
        self.begin_scope();

        for function in &module.items {
            let params: Vec<Type> = function.params.iter().map(|p| p.ty.lower()).collect();
            let ret = function
                .return_ty
                .as_ref()
                .map(|t| t.lower())
                .unwrap_or(Type::Unit);
            let ty = Type::Function {
                params: Rc::from(params),
                ret: Rc::from(ret),
            };
            self.declare(&function.name, ty, DeclarationKind::Function)?;
        }

        for function in &module.items {
            self.function(function)?;
        }
        self.end_scope();
        Ok(())
    }

    fn declare(
        &mut self,
        ident: &Identifier,
        ty: Type,
        kind: DeclarationKind,
    ) -> Result<DeclarationId> {
        let scope = self.scopes.front_mut().expect("Declaration without scope");
        let name = ident.symbol.clone();

        if scope.contains_key(&name) {
            return error(
                ident.node.span,
                format!("Name '{name}' is already declared in this scope"),
            );
        }

        let decl_id = self.semantics.declarations.len();
        let decl = Declaration {
            id: decl_id,
            ty,
            kind,
        };
        self.semantics.declarations.push(decl);

        scope.insert(name, decl_id);

        Ok(decl_id)
    }

    fn declare_local(
        &mut self,
        ident: &Identifier,
        ty: Type,
        func_id: DeclarationId,
    ) -> Result<()> {
        let decl_id = self.declare(ident, ty, DeclarationKind::Local(func_id))?;
        self.semantics.uses.insert(ident.node.id, decl_id);
        Ok(())
    }

    fn lookup(&mut self, ident: &Identifier) -> Result<DeclarationId> {
        let Some(decl_id) = self
            .scopes
            .iter()
            .find_map(|scope| scope.get(&ident.symbol))
            .copied()
        else {
            return error(ident.node.span, format!("Undeclared '{}'", ident.symbol));
        };

        Ok(decl_id)
    }

    fn function(&mut self, f: &Function) -> Result<()> {
        let decl_id = self.lookup(&f.name)?;
        self.semantics.uses.insert(f.name.node.id, decl_id);

        self.begin_scope();

        for Param { name, ty, .. } in f.params.iter() {
            self.declare_local(name, ty.lower(), decl_id)?
        }

        self.stmt(&f.body, decl_id)?;

        self.end_scope();

        Ok(())
    }

    fn stmt(&mut self, stmt: &Stmt, func_id: DeclarationId) -> Result<()> {
        match &stmt.kind {
            StmtKind::ExprStmt { expr } => {
                self.expr(expr)?;
            }
            StmtKind::Block { statements } => {
                for stmt in statements {
                    self.stmt(stmt, func_id)?;
                }
            }
            StmtKind::Declaration {
                name,
                ty,
                initializer,
            } => {
                self.declare_local(name, ty.lower(), func_id)?;
                self.expr(initializer)?;
            }
            StmtKind::Assignment { target, value } => {
                self.expr(target)?;
                if !target.is_variable() {
                    return error(target.node.span, "Invalid assignment target");
                }
                self.expr(value)?;
            }
            StmtKind::Return { expr } => {
                self.expr(expr)?;
            }
        }
        Ok(())
    }

    fn expr(&mut self, expr: &Expr) -> Result<Type> {
        let ty = match &expr.kind {
            ExprKind::Literal { kind } => match kind {
                LiteralKind::Int(_) => Type::Int,
                LiteralKind::Float(_) => Type::Float,
                LiteralKind::Bool(_) => Type::Bool,
            },
            ExprKind::Variable { name } => {
                let decl_id = self.lookup(name)?;
                self.semantics.uses.insert(name.node.id, decl_id);
                let decl = &self.semantics.declarations[decl_id];
                decl.ty.clone()
            }
            ExprKind::Unary { expr, .. } => self.expr(expr)?,
            ExprKind::Binary { left, right, .. } => {
                let left_ty = self.expr(left)?;
                let right_ty = self.expr(right)?;
                if left_ty != right_ty {
                    return error(expr.node.span, "Type mismatch in binary expression");
                }
                left_ty
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee)?;
                let ExprKind::Variable { name } = &callee.kind else {
                    return error(callee.node.span, "Invalid call callee");
                };
                for arg in args {
                    // TODO: typecheck args
                    self.expr(arg)?;
                }
                let decl_id = self.lookup(name)?;
                self.semantics.uses.insert(name.node.id, decl_id);
                let decl = &self.semantics.declarations[decl_id];
                decl.ty.clone()
            }
        };
        Ok(ty)
    }

    fn begin_scope(&mut self) {
        self.scopes.push_front(Default::default())
    }

    fn end_scope(&mut self) {
        self.scopes.pop_front();
    }
}

impl TypeRef {
    fn lower(&self) -> Type {
        match &self.kind {
            TypeKind::Int => Type::Int,
            TypeKind::Float => Type::Float,
            TypeKind::Bool => Type::Bool,
        }
    }
}

impl Expr {
    fn is_variable(&self) -> bool {
        matches!(self.kind, ExprKind::Variable { .. })
    }
}

pub fn semantic_analysis(module: &Module) -> Result<Semantics> {
    let mut resolver = Resolver {
        scopes: Default::default(),
        semantics: Default::default(),
    };

    resolver.module(module)?;

    Ok(resolver.semantics)
}

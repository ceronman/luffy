#[cfg(test)]
mod test;

use crate::ast::{
    BinOpKind, Expr, ExprKind, Function, Identifier, LiteralKind, Module, NodeId, Param, Stmt,
    StmtKind, Symbol, TypeKind, TypeRef,
};
use crate::error::{CompilerError, ErrorKind};
use crate::lexer::Span;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Display, Formatter};
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

impl Type {
    fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }
    fn is_int(&self) -> bool {
        matches!(self, Type::Int)
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Unit => write!(f, "Unit"),
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::Function { .. } => write!(f, "Function"), // TODO: improve
        }
    }
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

fn resolve_err<T: Debug>(span: Span, message: impl Into<String>) -> crate::parser::Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Resolve,
        msg: message.into(),
        span,
    })
}

fn type_err<T: Debug>(span: Span, message: impl Into<String>) -> crate::parser::Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Type,
        msg: message.into(),
        span,
    })
}

fn check_ty_match(span: Span, expected: &Type, actual: &Type) -> crate::parser::Result<()> {
    if actual == expected {
        Ok(())
    } else {
        type_err(
            span,
            format!("Type mismatch: expected '{expected}', found '{actual}'"),
        )
    }
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
            return resolve_err(
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

    fn lookup(&self, ident: &Identifier) -> Result<DeclarationId> {
        let Some(decl_id) = self
            .scopes
            .iter()
            .find_map(|scope| scope.get(&ident.symbol))
            .copied()
        else {
            return resolve_err(ident.node.span, format!("Undeclared '{}'", ident.symbol));
        };

        Ok(decl_id)
    }

    fn lookup_ty(&self, ident: &Identifier) -> Result<Type> {
        let decl_id = self.lookup(ident)?;
        let Some(decl) = self.semantics.declarations.get(decl_id) else {
            return resolve_err(ident.node.span, format!("Undeclared '{}'", ident.symbol));
        };
        Ok(decl.ty.clone())
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
                let var_ty = ty.lower();
                let init_ty = self.expr(initializer)?;
                check_ty_match(initializer.node.span, &var_ty, &init_ty)?;
                self.declare_local(name, var_ty, func_id)?;
            }
            StmtKind::Assignment { target, value } => {
                self.expr(target)?;
                let ExprKind::Variable { name } = &target.kind else {
                    return resolve_err(target.node.span, "Invalid assignment target");
                };
                let target_ty = self.lookup_ty(name)?;
                let expr_ty = self.expr(value)?;
                check_ty_match(value.node.span, &target_ty, &expr_ty)?;
            }
            StmtKind::Return { expr } => {
                let ty = self.expr(expr)?;
                let function_decl = &self.semantics.declarations[func_id];
                let Type::Function { ret, .. } = &function_decl.ty else {
                    return resolve_err(expr.node.span, "Return outside of function");
                };
                check_ty_match(expr.node.span, ret, &ty)?;
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
                self.lookup_ty(name)?
            }
            ExprKind::Unary { expr, .. } => self.expr(expr)?,
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.expr(left)?;
                let right_ty = self.expr(right)?;
                check_ty_match(right.node.span, &left_ty, &right_ty)?;
                match op.kind {
                    BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul | BinOpKind::Div => {
                        if !left_ty.is_numeric() {
                            return type_err(
                                left.node.span,
                                "Operator requires numeric type".to_string(),
                            );
                        }
                        left_ty
                    }

                    BinOpKind::Mod => {
                        if !left_ty.is_int() {
                            return type_err(
                                expr.node.span,
                                format!("Modulo operator is not implemented for {left_ty}"),
                            );
                        }
                        left_ty
                    }

                    BinOpKind::Eq | BinOpKind::Ne => Type::Bool,

                    BinOpKind::Ge | BinOpKind::Gt | BinOpKind::Le | BinOpKind::Lt => {
                        if !left_ty.is_numeric() {
                            return type_err(
                                left.node.span,
                                "Operator requires numeric type".to_string(),
                            );
                        }
                        Type::Bool
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                let Type::Function { params, ret } = self.expr(callee)? else {
                    return type_err(
                        callee.node.span,
                        "Invalid function call: callee is not a function",
                    );
                };
                for (arg, param_ty) in args.iter().zip(params.iter()) {
                    let arg_ty = self.expr(arg)?;
                    check_ty_match(arg.node.span, param_ty, &arg_ty)?;
                }
                (*ret).clone()
            }
        };
        self.semantics.expr_types.insert(expr.node.id, ty.clone());
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

pub fn semantic_analysis(module: &Module) -> Result<Semantics> {
    let mut resolver = Resolver {
        scopes: Default::default(),
        semantics: Default::default(),
    };

    resolver.module(module)?;

    Ok(resolver.semantics)
}

#[cfg(test)]
mod test;

use crate::ast::{
    BinOpKind, Block, BlockKind, Expr, ExprKind, Identifier, ItemKind, LiteralKind, Module, NodeId,
    Param, Stmt, StmtKind, Symbol, TypeKind, TypeRef, UnOpKind,
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
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }
    pub fn is_int(&self) -> bool {
        matches!(self, Type::Int)
    }
    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }
    pub fn is_unit(&self) -> bool {
        matches!(self, Type::Unit)
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

        for item in &module.items {
            match &item.kind {
                ItemKind::Function {
                    name,
                    params,
                    return_ty,
                    ..
                }
                | ItemKind::Import {
                    name,
                    params,
                    return_ty,
                } => {
                    let params: Vec<Type> = params.iter().map(|p| p.ty.lower()).collect();
                    let ret = return_ty.as_ref().map(|t| t.lower()).unwrap_or(Type::Unit);
                    let ty = Type::Function {
                        params: Rc::from(params),
                        ret: Rc::from(ret),
                    };
                    self.declare(name, ty, DeclarationKind::Function)?;
                }
            }
        }

        for function in &module.items {
            match &function.kind {
                ItemKind::Function {
                    name, params, body, ..
                } => {
                    self.function(name, params, body)?;
                }
                ItemKind::Import { name, .. } => {
                    let decl_id = self.lookup(name)?;
                    self.semantics.uses.insert(name.node.id, decl_id);
                }
            }
        }
        self.end_scope();
        Ok(())
    }

    fn function(&mut self, name: &Identifier, params: &[Param], body: &Block) -> Result<()> {
        let decl_id = self.lookup(name)?;
        self.semantics.uses.insert(name.node.id, decl_id);

        self.begin_scope();
        for Param {
            name: param_name,
            ty,
            ..
        } in params.iter()
        {
            self.declare_local(param_name, ty.lower(), decl_id)?
        }
        self.block(body, decl_id)?;
        self.end_scope();

        Ok(())
    }

    fn block(&mut self, block: &Block, func_id: DeclarationId) -> Result<()> {
        match &block.kind {
            BlockKind::Braces { statements } => {
                for stmt in statements {
                    self.stmt(stmt, func_id)?;
                }
                let function_decl = &self.semantics.declarations[func_id];
                let Type::Function { ret, .. } = &function_decl.ty else {
                    return resolve_err(block.node.span, "Return outside of function");
                };
                let ends_with_return = matches!(
                    statements.last().map(|s| &s.kind),
                    Some(StmtKind::Return { .. })
                );
                if !ret.is_unit() && !ends_with_return {
                    let end = block.node.span.end;
                    return type_err(Span::new(end - 1, end), "Missing return statement");
                }
            }
            BlockKind::Expr { expr } => {
                let ty = self.expr(expr, func_id)?;
                let function_decl = &self.semantics.declarations[func_id];
                let Type::Function { ret, .. } = &function_decl.ty else {
                    return resolve_err(expr.node.span, "Return outside of function");
                };
                check_ty_match(expr.node.span, ret, &ty)?;
            }
        }
        Ok(())
    }

    fn stmt(&mut self, stmt: &Stmt, func_id: DeclarationId) -> Result<Type> {
        let ty = match &stmt.kind {
            StmtKind::ExprStmt { expr } => {
                if let ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } = &expr.kind
                {
                    // TODO: Figure out how to avoid this duplication
                    self.check_condition(condition, func_id, "if")?;
                    self.block_type(then_branch, func_id)?;
                    if let Some(else_branch) = else_branch {
                        self.block_type(else_branch, func_id)?;
                    }
                    self.semantics.expr_types.insert(expr.node.id, Type::Unit);
                    Type::Unit
                } else {
                    self.expr(expr, func_id)?
                }
            }
            StmtKind::Block { statements } => {
                // TODO: This is very similar to `self.block_type` Maybe remove this alltogether?
                let mut ty = Type::Unit;
                for stmt in statements {
                    ty = self.stmt(stmt, func_id)?;
                }
                ty
            }
            StmtKind::Declaration {
                name,
                ty,
                initializer,
            } => {
                let var_ty = ty.lower();
                let init_ty = self.expr(initializer, func_id)?;
                check_ty_match(initializer.node.span, &var_ty, &init_ty)?;
                self.declare_local(name, var_ty, func_id)?;
                Type::Unit
            }
            StmtKind::Assignment { target, value } => {
                self.expr(target, func_id)?;
                let ExprKind::Variable { name } = &target.kind else {
                    return resolve_err(target.node.span, "Invalid assignment target");
                };
                let target_ty = self.lookup_ty(name)?;
                let expr_ty = self.expr(value, func_id)?;
                check_ty_match(value.node.span, &target_ty, &expr_ty)?;
                Type::Unit
            }
            StmtKind::While { condition, body } => {
                self.check_condition(condition, func_id, "while")?;
                self.block_type(body, func_id)?;
                Type::Unit
            }
            StmtKind::Return { expr } => {
                let ty = self.expr(expr, func_id)?;
                let function_decl = &self.semantics.declarations[func_id];
                let Type::Function { ret, .. } = &function_decl.ty else {
                    return resolve_err(expr.node.span, "Return outside of function");
                };
                check_ty_match(expr.node.span, ret, &ty)?;
                Type::Unit
            }
        };
        self.semantics.expr_types.insert(stmt.node.id, Type::Unit);
        Ok(ty)
    }

    fn expr(&mut self, expr: &Expr, func_id: DeclarationId) -> Result<Type> {
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
            ExprKind::Unary { expr, op } => {
                let expr_ty = self.expr(expr, func_id)?;
                match op.kind {
                    UnOpKind::Neg => {
                        if !expr_ty.is_numeric() {
                            return type_err(
                                expr.node.span,
                                "Operator requires numeric type".to_string(),
                            );
                        }
                    }
                    UnOpKind::Not => {
                        if !expr_ty.is_bool() {
                            return type_err(
                                expr.node.span,
                                "Operator requires boolean type".to_string(),
                            );
                        }
                    }
                }
                expr_ty
            }
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.expr(left, func_id)?;
                let right_ty = self.expr(right, func_id)?;
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

                    BinOpKind::And | BinOpKind::Or => {
                        if !left_ty.is_bool() {
                            return type_err(
                                left.node.span,
                                "Logical operator requires boolean type".to_string(),
                            );
                        }
                        Type::Bool
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                let Type::Function { params, ret } = self.expr(callee, func_id)? else {
                    return type_err(
                        callee.node.span,
                        "Invalid function call: callee is not a function",
                    );
                };
                for (arg, param_ty) in args.iter().zip(params.iter()) {
                    let arg_ty = self.expr(arg, func_id)?;
                    check_ty_match(arg.node.span, param_ty, &arg_ty)?;
                }
                (*ret).clone()
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_condition(condition, func_id, "if")?;
                let Some(else_branch) = else_branch else {
                    return type_err(
                        expr.node.span,
                        "'if' must have both main and 'else' branches when used as an expression.",
                    );
                };
                let then_ty = self.block_type(then_branch, func_id)?;
                let else_ty = self.block_type(else_branch, func_id)?;
                check_ty_match(expr.node.span, &then_ty, &else_ty)?;
                then_ty
            }
        };
        self.semantics.expr_types.insert(expr.node.id, ty.clone());
        Ok(ty)
    }

    fn check_condition(
        &mut self,
        condition: &Expr,
        func_id: DeclarationId,
        keyword: &str,
    ) -> Result<()> {
        let cond_ty = self.expr(condition, func_id)?;
        if !cond_ty.is_bool() {
            return type_err(
                condition.node.span,
                format!("Condition of '{keyword}' must be 'Bool', found '{cond_ty}'"),
            );
        }
        Ok(())
    }

    fn block_type(&mut self, block: &Block, func_id: DeclarationId) -> Result<Type> {
        self.begin_scope();
        let ty = match &block.kind {
            BlockKind::Braces { statements } => {
                let mut result = Type::Unit;
                for stmt in statements {
                    result = self.stmt(stmt, func_id)?;
                }
                result
            }
            BlockKind::Expr { expr } => self.expr(expr, func_id)?,
        };
        self.end_scope();
        Ok(ty)
    }

    fn begin_scope(&mut self) {
        self.scopes.push_front(Default::default())
    }

    fn end_scope(&mut self) {
        self.scopes.pop_front();
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

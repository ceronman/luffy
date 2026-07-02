#[cfg(test)]
mod test;

use crate::ast::{
    BinOpKind, Block, BlockKind, Expr, ExprKind, Identifier, ItemKind, LiteralKind, MappingField,
    Module, Node, NodeId, Param, Stmt, StmtKind, TypeRef, UnOpKind,
};
use crate::error::{CompilerError, internal_err, resolve_err, type_err};
use crate::source::{Span, Symbol};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;

pub type DeclarationId = usize;

pub struct Declaration {
    pub id: DeclarationId,
    pub name: Symbol,
    pub ty: Type,
    pub kind: DeclarationKind,
}

pub enum DeclarationKind {
    Local { func_id: DeclarationId },
    Function,
    Import,
    Struct,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Unit,
    Int,
    Float,
    Bool,
    Never,
    Array {
        ty: Rc<Type>,
    },
    Struct {
        name: Symbol,
        fields: Rc<[StructField]>,
    },
    Reference {
        name: Symbol,
    },
    Function {
        params: Rc<[Type]>,
        ret: Rc<Type>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub name: Symbol,
    pub ty: Rc<Type>,
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
    pub fn is_never(&self) -> bool {
        matches!(self, Type::Never)
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Unit => write!(f, "Unit"),
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::Never => write!(f, "Never"),
            Type::Array { ty } => write!(f, "Array[{ty}]"),
            Type::Reference { name } => write!(f, "{name}",),
            Type::Struct { .. } => write!(f, "Struct"), // TODO: improve
            Type::Function { .. } => write!(f, "Function"), // TODO: improve
        }
    }
}

#[derive(Default)]
pub struct Semantics {
    pub expr_types: HashMap<NodeId, Type>,
    pub struct_types: HashMap<Symbol, Type>,
    pub declarations: Vec<Declaration>,
    pub uses: HashMap<NodeId, DeclarationId>,
}

type Scope = HashMap<Symbol, DeclarationId>;

#[derive(Default)]
struct Resolver {
    scopes: VecDeque<Scope>,
    semantics: Semantics,
    loop_depth: u32,
    incomplete_types: HashSet<Symbol>,
}

pub type Result<T> = std::result::Result<T, CompilerError>;

fn unify_ty(span: Span, expected: &Type, actual: &Type) -> crate::parser::Result<Type> {
    if actual == expected {
        Ok(actual.clone())
    } else if actual.is_never() {
        Ok(expected.clone())
    } else if expected.is_never() {
        Ok(actual.clone())
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
                    let params: Vec<Type> = params
                        .iter()
                        .map(|p| self.ty_ref(&p.ty))
                        .collect::<Result<_>>()?;
                    let ret = return_ty
                        .as_ref()
                        .map(|t| self.ty_ref(t))
                        .unwrap_or(Ok(Type::Unit))?;
                    let ty = Type::Function {
                        params: Rc::from(params),
                        ret: Rc::from(ret),
                    };
                    let kind = match &item.kind {
                        ItemKind::Function { .. } => DeclarationKind::Function,
                        ItemKind::Import { .. } => DeclarationKind::Import,
                        _ => unreachable!(),
                    };
                    self.declare(name, ty, kind)?;
                }
                ItemKind::Struct { name, .. } => {
                    self.incomplete_types.insert(name.symbol.clone());
                }
            }
        }

        for item in &module.items {
            if let ItemKind::Struct { name, fields } = &item.kind {
                let mut ty_fields = Vec::new();
                for field in fields {
                    let name = field.name.symbol.clone();
                    let ty = self.ty_ref(&field.ty)?;
                    ty_fields.push(StructField {
                        name,
                        ty: Rc::from(ty),
                    });
                }
                let ty = Type::Struct {
                    name: name.symbol.clone(),
                    fields: Rc::from(ty_fields),
                };
                self.declare(name, ty.clone(), DeclarationKind::Struct)?;
                let Some(option) = self.incomplete_types.take(&name.symbol) else {
                    return internal_err(
                        name.node.span,
                        "Struct definition `{}` does not have an incomplete type",
                    );
                };
                self.semantics.struct_types.insert(option, ty);
            }
        }

        debug_assert!(self.incomplete_types.is_empty());

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
                _ => {}
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
            let ty = self.ty_ref(ty)?;
            self.declare_local(param_name, ty, decl_id)?
        }
        let ret_ty = self.function_ret_ty(body.node, decl_id)?;
        match &body.kind {
            BlockKind::Braces { statements } => {
                let block_ty = self.statements(statements, decl_id, None)?;
                // TODO: Improve this logic to be more clear
                if !ret_ty.is_unit() && !block_ty.is_never() {
                    return type_err(body.node.span, "Missing return statement");
                }
            }
            BlockKind::Expr { expr } => {
                let ty = self.expr(expr, decl_id, Some(&ret_ty))?;
                unify_ty(expr.node.span, &ret_ty, &ty)?;
            }
        }
        self.end_scope();

        Ok(())
    }

    fn stmt(&mut self, stmt: &Stmt, func_id: DeclarationId) -> Result<()> {
        match &stmt.kind {
            StmtKind::ExprStmt { expr } => {
                self.expr(expr, func_id, None)?;
            }
            StmtKind::Declaration {
                name,
                ty,
                initializer,
            } => {
                let var_ty = self.ty_ref(ty)?;
                if let Some(initializer) = initializer {
                    let init_ty = self.expr(initializer, func_id, Some(&var_ty))?;
                    unify_ty(initializer.node.span, &var_ty, &init_ty)?;
                }
                self.declare_local(name, var_ty, func_id)?;
            }
            StmtKind::While { condition, body } => {
                self.check_condition(condition, func_id, "while")?;
                self.loop_depth += 1;
                self.block(body, func_id, None)?;
                self.loop_depth -= 1;
            }
        }
        Ok(())
    }
    fn expr(
        &mut self,
        expr: &Expr,
        func_id: DeclarationId,
        expected_ty: Option<&Type>,
    ) -> Result<Type> {
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
            ExprKind::Collection { elements } => {
                let Some(collection_ty) = expected_ty else {
                    return type_err(
                        expr.node.span,
                        "Not enough information to infer type of collection",
                    );
                };
                let Type::Array { ty: inner } = collection_ty else {
                    return type_err(
                        expr.node.span,
                        format!("Type mismatch: expected '{collection_ty}', found collection"),
                    );
                };
                for element in elements {
                    let element_ty = self.expr(element, func_id, Some(inner))?;
                    unify_ty(element.node.span, inner, &element_ty)?;
                }
                collection_ty.clone()
            }
            ExprKind::Mapping { fields } => {
                let Some(mapping_ty) = expected_ty else {
                    return type_err(
                        expr.node.span,
                        "Not enough information to infer type of mapping",
                    );
                };

                let Type::Reference { name } = mapping_ty else {
                    return type_err(
                        expr.node.span,
                        format!("Type mismatch: expected '{mapping_ty}', found mapping"),
                    );
                };
                let Some(Type::Struct {
                    fields: ty_fields, ..
                }) = self.semantics.struct_types.get(name)
                else {
                    return type_err(expr.node.span, format!("Unknown type '{name}'"));
                };

                let mut ty_fields = ty_fields.to_vec();

                // TODO: is there a way to optimize this?
                let mut seen = HashSet::new();
                for MappingField { key, value, .. } in fields {
                    if seen.contains(&key.symbol) {
                        return type_err(
                            key.node.span,
                            format!("Duplicate field '{}'", key.symbol),
                        );
                    }
                    let Some(ty_field) = ty_fields.iter().position(|f| f.name == key.symbol) else {
                        return type_err(
                            key.node.span,
                            format!("Field '{}' does not exist in type '{name}'", key.symbol),
                        );
                    };
                    let ty_field = ty_fields.swap_remove(ty_field);
                    let value_ty = self.expr(value, func_id, Some(&ty_field.ty))?;
                    unify_ty(value.node.span, &ty_field.ty, &value_ty)?;
                    seen.insert(ty_field.name);
                }

                if let Some(missing_field) = ty_fields.first() {
                    return type_err(
                        expr.node.span,
                        format!(
                            "Type mismatch: mapping is missing field '{}'",
                            missing_field.name
                        ),
                    );
                }

                mapping_ty.clone()
            }
            ExprKind::Unary { expr, op } => {
                let expr_ty = self.expr(expr, func_id, None)?;
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
                let left_ty = self.expr(left, func_id, None)?;
                let right_ty = self.expr(right, func_id, None)?;
                let result_ty = unify_ty(right.node.span, &left_ty, &right_ty)?;
                match op.kind {
                    BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul | BinOpKind::Div => {
                        if !left_ty.is_numeric() {
                            return type_err(
                                left.node.span,
                                "Operator requires numeric type".to_string(),
                            );
                        }
                        result_ty
                    }

                    BinOpKind::Mod => {
                        if !left_ty.is_int() {
                            return type_err(
                                expr.node.span,
                                format!("Modulo operator is not implemented for {left_ty}"),
                            );
                        }
                        result_ty
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
                let Type::Function { params, ret } = self.expr(callee, func_id, None)? else {
                    return type_err(
                        callee.node.span,
                        "Invalid function call: callee is not a function",
                    );
                };
                for (param_ty, arg) in params.iter().zip(args.iter()) {
                    let arg_ty = self.expr(arg, func_id, Some(param_ty))?;
                    let unified_ty = unify_ty(arg.node.span, param_ty, &arg_ty)?;
                    self.semantics.expr_types.insert(arg.node.id, unified_ty);
                }
                (*ret).clone()
            }
            ExprKind::Assignment { target, value } => {
                let target_ty = self.expr(target, func_id, None)?;
                match &target.kind {
                    ExprKind::Variable { .. } => {}
                    ExprKind::Index { .. } => {}
                    ExprKind::Field { .. } => {}
                    _ => return resolve_err(target.node.span, "Invalid assignment target"),
                };
                let expr_ty = self.expr(value, func_id, Some(&target_ty))?;
                unify_ty(value.node.span, &target_ty, &expr_ty)?;
                Type::Unit
            }
            ExprKind::Index { expr, index } => {
                let expr_ty = self.expr(expr, func_id, None)?;
                let Type::Array { ty: inner } = expr_ty else {
                    return type_err(
                        expr.node.span,
                        format!("Type mismatch: expected Array, found '{expr_ty}'"),
                    );
                };
                let index_ty = self.expr(index, func_id, None)?;
                let Type::Int = index_ty else {
                    return type_err(
                        index.node.span,
                        format!("Type mismatch: index expected as Int, found '{index_ty}'"),
                    );
                };
                (*inner).clone()
            }
            ExprKind::Field { expr, field } => {
                let expr_ty = self.expr(expr, func_id, None)?;
                let Type::Reference { name } = expr_ty else {
                    return type_err(
                        expr.node.span,
                        format!("Type mismatch: expected struct, found '{expr_ty}'"),
                    );
                };
                let Some(Type::Struct {
                    name: ty_name,
                    fields: ty_fields,
                }) = self.semantics.struct_types.get(&name)
                else {
                    return type_err(
                        field.node.span,
                        format!("Type mismatch: struct '{name}' not found"),
                    );
                };
                debug_assert!(name == *ty_name);
                let Some(f) = ty_fields.iter().find(|&f| f.name == field.symbol) else {
                    return type_err(
                        field.node.span,
                        format!(
                            "Type mismatch: field '{}' not found in struct '{ty_name}'",
                            field.symbol
                        ),
                    );
                };
                (*f.ty).clone()
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_condition(condition, func_id, "if")?;
                let then_ty = self.block(then_branch, func_id, expected_ty)?;
                if let Some(else_branch) = else_branch {
                    let else_ty = self.block(else_branch, func_id, expected_ty)?;
                    unify_ty(else_branch.node.span, &then_ty, &else_ty)?
                } else {
                    Type::Unit
                }
            }
            ExprKind::Break => {
                if self.loop_depth == 0 {
                    return resolve_err(expr.node.span, "'break' outside of a loop");
                }
                Type::Never
            }
            ExprKind::Continue => {
                if self.loop_depth == 0 {
                    return resolve_err(expr.node.span, "'continue' outside of a loop");
                }
                Type::Never
            }
            ExprKind::Return { expr: inner } => {
                let ret_ty = self.function_ret_ty(inner.node, func_id)?;
                let inner_ty = self.expr(inner, func_id, Some(&ret_ty))?;
                unify_ty(inner.node.span, &ret_ty, &inner_ty)?;
                Type::Never
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
        let cond_ty = self.expr(condition, func_id, None)?;
        if !cond_ty.is_bool() {
            return type_err(
                condition.node.span,
                format!("Condition of '{keyword}' must be 'Bool', found '{cond_ty}'"),
            );
        }
        Ok(())
    }

    fn block(
        &mut self,
        block: &Block,
        func_id: DeclarationId,
        expected_ty: Option<&Type>,
    ) -> Result<Type> {
        self.begin_scope();
        let ty = match &block.kind {
            BlockKind::Braces { statements } => {
                self.statements(statements, func_id, expected_ty)?
            }
            BlockKind::Expr { expr } => self.expr(expr, func_id, expected_ty)?,
        };
        self.end_scope();
        self.semantics.expr_types.insert(block.node.id, ty.clone());
        Ok(ty)
    }

    fn statements(
        &mut self,
        statements: &[Stmt],
        func_id: DeclarationId,
        expected_ty: Option<&Type>,
    ) -> Result<Type> {
        let mut result = Type::Unit;
        for (i, stmt) in statements.iter().enumerate() {
            if let StmtKind::ExprStmt { expr } = &stmt.kind {
                let expected_ty = if i == statements.len() - 1 {
                    expected_ty
                } else {
                    None
                };
                result = self.expr(expr, func_id, expected_ty)?
            } else {
                self.stmt(stmt, func_id)?;
                result = Type::Unit;
            }
        }
        Ok(result)
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
            name: name.clone(),
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
        let decl_id = self.declare(ident, ty, DeclarationKind::Local { func_id })?;
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

    fn function_ret_ty(&self, node: Node, func_id: DeclarationId) -> Result<Type> {
        let Some(function_decl) = self.semantics.declarations.get(func_id) else {
            return resolve_err(node.span, "Enclosing function not found");
        };
        let Type::Function { ret, .. } = function_decl.ty.clone() else {
            return resolve_err(node.span, "Enclosing function not found");
        };
        Ok(Rc::unwrap_or_clone(ret))
    }

    fn ty_ref(&self, type_ref: &TypeRef) -> Result<Type> {
        match type_ref.name.symbol.as_str() {
            "Int" => Ok(Type::Int),
            "Float" => Ok(Type::Float),
            "Bool" => Ok(Type::Bool),
            "Unit" => Ok(Type::Unit),
            "Never" => Ok(Type::Never),
            "Array" => match type_ref.args.as_slice() {
                [inner] => {
                    let inner = self.ty_ref(inner)?;
                    Ok(Type::Array {
                        ty: Rc::from(inner),
                    })
                }
                _ => resolve_err(
                    type_ref.node.span,
                    format!(
                        "Array type requires one type argument, {} given",
                        type_ref.args.len()
                    ),
                ),
            },
            name => {
                if self.incomplete_types.contains(&type_ref.name.symbol)
                    || self
                        .semantics
                        .struct_types
                        .contains_key(&type_ref.name.symbol)
                {
                    Ok(Type::Reference { name: name.into() })
                } else {
                    resolve_err(type_ref.node.span, format!("Unknown type `{name}`"))
                }
            }
        }
    }
}

pub fn semantic_analysis(module: &Module) -> Result<Semantics> {
    let mut resolver = Resolver::default();

    resolver.module(module)?;

    #[cfg(debug_assertions)]
    assert_fully_typed(module, &resolver.semantics);
    debug_assert!(resolver.incomplete_types.is_empty());

    Ok(resolver.semantics)
}

#[cfg(debug_assertions)]
fn assert_fully_typed(module: &Module, semantics: &Semantics) {
    fn check_expr(expr: &Expr, semantics: &Semantics) {
        semantics.expr_types.get(&expr.node.id).unwrap_or_else(|| {
            panic!(
                "expression {} (span {:?}) has no type assigned after semantic analysis",
                expr.node.id, expr.node.span
            )
        });
        match &expr.kind {
            ExprKind::Literal { .. }
            | ExprKind::Variable { .. }
            | ExprKind::Break
            | ExprKind::Continue => {}
            ExprKind::Collection { elements } => {
                for element in elements {
                    check_expr(element, semantics);
                }
            }
            ExprKind::Mapping { fields } => {
                for field in fields {
                    check_expr(&field.value, semantics);
                }
            }
            ExprKind::Unary { expr, .. } => check_expr(expr, semantics),
            ExprKind::Binary { left, right, .. } => {
                check_expr(left, semantics);
                check_expr(right, semantics);
            }
            ExprKind::Call { callee, args } => {
                check_expr(callee, semantics);
                for arg in args {
                    check_expr(arg, semantics);
                }
            }
            ExprKind::Assignment { target, value } => {
                check_expr(target, semantics);
                check_expr(value, semantics);
            }
            ExprKind::Index { expr, index } => {
                check_expr(expr, semantics);
                check_expr(index, semantics);
            }
            ExprKind::Field { expr, .. } => {
                check_expr(expr, semantics);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                check_expr(condition, semantics);
                check_block(then_branch, semantics);
                if let Some(else_branch) = else_branch {
                    check_block(else_branch, semantics);
                }
            }
            ExprKind::Return { expr } => check_expr(expr, semantics),
        }
    }

    fn check_block(block: &Block, semantics: &Semantics) {
        match &block.kind {
            BlockKind::Braces { statements } => {
                for stmt in statements {
                    check_stmt(stmt, semantics);
                }
            }
            BlockKind::Expr { expr } => check_expr(expr, semantics),
        }
    }

    fn check_stmt(stmt: &Stmt, semantics: &Semantics) {
        match &stmt.kind {
            StmtKind::ExprStmt { expr } => check_expr(expr, semantics),
            StmtKind::Declaration { initializer, .. } => {
                if let Some(initializer) = initializer {
                    check_expr(initializer, semantics);
                }
            }
            StmtKind::While { condition, body } => {
                check_expr(condition, semantics);
                check_block(body, semantics);
            }
        }
    }

    for item in &module.items {
        if let ItemKind::Function { body, .. } = &item.kind {
            check_block(body, semantics);
        }
    }
}

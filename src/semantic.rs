use crate::ast::{
    Expr, ExprKind, Function, Identifier, Module, NodeId, Param, Stmt, StmtKind, Symbol,
};
use crate::error::{CompilerError, ErrorKind};
use crate::lexer::Span;
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;

pub type DeclarationId = u32;

pub struct Declaration {
    pub id: DeclarationId,
    pub kind: DeclarationKind,
}

pub enum DeclarationKind {
    Local(DeclarationId),
    Function,
}

#[derive(Default)]
pub struct Semantics {
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
            self.function(function)?;
        }
        self.end_scope();
        Ok(())
    }

    fn declare(&mut self, ident: &Identifier, kind: DeclarationKind) -> Result<DeclarationId> {
        let scope = self.scopes.front_mut().expect("Declaration without scope");
        let name = ident.symbol.clone();

        if scope.contains_key(&name) {
            return error(
                ident.node.span,
                format!("Name '{name}' is already declared in this scope"),
            );
        }

        let decl_id = self.semantics.declarations.len() as u32;
        let decl = Declaration { id: decl_id, kind };
        self.semantics.declarations.push(decl);

        scope.insert(name, decl_id);

        Ok(decl_id)
    }

    fn declare_local(&mut self, ident: &Identifier, func_id: DeclarationId) -> Result<()> {
        let decl_id = self.declare(ident, DeclarationKind::Local(func_id))?;
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
        let decl_id = self.declare(&f.name, DeclarationKind::Function)?;
        self.semantics.uses.insert(f.name.node.id, decl_id);

        self.begin_scope();

        for Param { name, .. } in f.params.iter() {
            self.declare_local(name, decl_id)?
        }

        self.stmt(&f.body, decl_id)?;

        self.end_scope();

        Ok(())
    }

    fn stmt(&mut self, stmt: &Stmt, func_id: DeclarationId) -> Result<()> {
        match &stmt.kind {
            StmtKind::ExprStmt { expr } => {
                self.expr(expr)?
            },
            StmtKind::Block { statements } => {
                for stmt in statements {
                    self.stmt(stmt, func_id)?;
                }
            }
            StmtKind::Declaration {
                name, initializer, ..
            } => {
                self.declare_local(name, func_id)?;
                self.expr(initializer)?;
            }
            StmtKind::Assignment { target, value } => {
                self.expr(target)?;
                if !matches!(target.kind, ExprKind::Variable { .. }) {
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

    fn expr(&mut self, expr: &Expr) -> Result<()> {
        match &expr.kind {
            ExprKind::Literal { .. } => {}
            ExprKind::Variable { name } => {
                let decl_id = self.lookup(name)?;
                self.semantics.uses.insert(name.node.id, decl_id);
            }
            ExprKind::Unary { expr, .. } => {
                self.expr(expr)?;
            }
            ExprKind::Binary { left, right, .. } => {
                self.expr(left)?;
                self.expr(right)?;
            }
            ExprKind::Call { .. } => todo!(),
        }
        Ok(())
    }

    fn begin_scope(&mut self) {
        self.scopes.push_front(Default::default())
    }

    fn end_scope(&mut self) {
        self.scopes.pop_front();
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

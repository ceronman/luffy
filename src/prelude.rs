//! The Luffy prelude: library functions written in Luffy itself
//! (`prelude.luffy`), spliced into the user's module before semantic
//! analysis.
//!
//! Injection is gated on use so that programs only pay for what they
//! reference: a prelude item is included when its name occurs in the user
//! module (or, transitively, in an already-included prelude item), and a
//! user top-level declaration with the same name shadows the prelude
//! version. The prelude is parsed with node ids starting after the user
//! module's, keeping ids unique across the spliced module.

use crate::ast::{Block, BlockKind, Expr, ExprKind, Item, ItemKind, Module, Stmt, StmtKind};
use crate::parser;
use crate::source::Symbol;
use std::collections::HashSet;

pub const SOURCE: &str = include_str!("prelude.luffy");

/// Parses a user module and splices in the prelude items it references.
pub fn parse_with_prelude(source: &str) -> parser::Result<Module> {
    let (mut module, next_id) = parser::parse_with_start_id(source, 0)?;
    inject(&mut module, next_id);
    Ok(module)
}

fn inject(module: &mut Module, next_id: u32) {
    // A parse error here is a compiler bug, not a user error: the prelude
    // ships with the compiler and is checked by its tests.
    let (prelude, _) = parser::parse_with_start_id(SOURCE, next_id)
        .unwrap_or_else(|e| panic!("prelude failed to parse: {e:?}"));

    let user_names: HashSet<Symbol> = module
        .items
        .iter()
        .map(|item| item_name(item).clone())
        .collect();

    let mut referenced = HashSet::new();
    for item in &module.items {
        collect_item(item, &mut referenced);
    }

    // Fixpoint: pull in every referenced prelude item, then whatever those
    // items reference, until nothing new is added.
    let mut remaining = prelude.items;
    let mut included = Vec::new();
    loop {
        let mut progressed = false;
        let mut i = 0;
        while i < remaining.len() {
            let name = item_name(&remaining[i]);
            if referenced.contains(name) && !user_names.contains(name) {
                let item = remaining.remove(i);
                collect_item(&item, &mut referenced);
                included.push(item);
                progressed = true;
            } else {
                i += 1;
            }
        }
        if !progressed {
            break;
        }
    }

    // Appended after the user items, so user function indices are unchanged.
    module.items.extend(included);
}

fn item_name(item: &Item) -> &Symbol {
    match &item.kind {
        ItemKind::Function { name, .. }
        | ItemKind::Struct { name, .. }
        | ItemKind::Import { name, .. } => &name.symbol,
    }
}

/// Collects every identifier used as a variable (which includes callees)
/// into `out`. Over-collection — e.g. a local variable sharing a prelude
/// function's name — at worst injects an unused item.
fn collect_item(item: &Item, out: &mut HashSet<Symbol>) {
    if let ItemKind::Function { body, .. } = &item.kind {
        collect_block(body, out);
    }
}

fn collect_block(block: &Block, out: &mut HashSet<Symbol>) {
    match &block.kind {
        BlockKind::Braces { statements } => {
            for stmt in statements {
                collect_stmt(stmt, out);
            }
        }
        BlockKind::Expr { expr } => collect_expr(expr, out),
    }
}

fn collect_stmt(stmt: &Stmt, out: &mut HashSet<Symbol>) {
    match &stmt.kind {
        StmtKind::ExprStmt { expr } => collect_expr(expr, out),
        StmtKind::Declaration { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_expr(initializer, out);
            }
        }
        StmtKind::While { condition, body } => {
            collect_expr(condition, out);
            collect_block(body, out);
        }
    }
}

fn collect_expr(expr: &Expr, out: &mut HashSet<Symbol>) {
    match &expr.kind {
        ExprKind::Literal { .. } | ExprKind::Break | ExprKind::Continue => {}
        ExprKind::Variable { name } => {
            out.insert(name.symbol.clone());
        }
        ExprKind::Collection { elements } => {
            for element in elements {
                collect_expr(element, out);
            }
        }
        ExprKind::Mapping { fields } => {
            for field in fields {
                collect_expr(&field.value, out);
            }
        }
        ExprKind::Unary { expr, .. } => collect_expr(expr, out),
        ExprKind::Binary { left, right, .. } => {
            collect_expr(left, out);
            collect_expr(right, out);
        }
        ExprKind::Call { callee, args } => {
            collect_expr(callee, out);
            for arg in args {
                collect_expr(arg, out);
            }
        }
        ExprKind::Assignment { target, value } => {
            collect_expr(target, out);
            collect_expr(value, out);
        }
        ExprKind::Index { expr, index } => {
            collect_expr(expr, out);
            collect_expr(index, out);
        }
        ExprKind::Field { expr, .. } => collect_expr(expr, out),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr(condition, out);
            collect_block(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_block(else_branch, out);
            }
        }
        ExprKind::Return { expr } => collect_expr(expr, out),
    }
}

#[cfg(test)]
mod test {
    use super::SOURCE;
    use crate::{parser, semantic};

    #[test]
    fn prelude_parses_and_typechecks() {
        // Users never see errors pointing into the prelude, so it must be
        // fully valid on its own.
        let module = parser::parse(SOURCE).expect("prelude failed to parse");
        semantic::semantic_analysis(&module).expect("prelude failed to type-check");
    }
}

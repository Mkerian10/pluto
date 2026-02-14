/// Composition utilities for common visitor patterns.
///
/// This module provides helper functions that reduce boilerplate when writing
/// visitors for common tasks like finding, counting, and collecting nodes.
///
/// # Examples
///
/// ```
/// use crate::visit::composers::*;
/// use crate::parser::ast::*;
/// use crate::span::Spanned;
///
/// // Check if an expression contains any propagate operators
/// let has_propagate = contains_expr(&expr, |e| matches!(e, Expr::Propagate { .. }));
///
/// // Count all yield statements in a block
/// let yield_count = count_stmts(&block, |s| matches!(s, Stmt::Yield { .. }));
///
/// // Collect all identifier names
/// let idents = collect_exprs(&expr, |e| {
///     if let Expr::Ident(name) = e {
///         Some(name.clone())
///     } else {
///         None
///     }
/// });
/// ```

use crate::parser::ast::*;
use crate::span::Spanned;
use crate::visit::{walk_expr, walk_stmt, Visitor};
use std::collections::HashSet;

// ============================================================================
// Detection / Predicate Helpers
// ============================================================================

/// Check if an expression tree contains any expression matching a predicate.
///
/// Short-circuits on first match for efficiency.
///
/// # Examples
///
/// ```
/// // Check for error propagation
/// let has_propagate = contains_expr(&expr, |e| matches!(e, Expr::Propagate { .. }));
///
/// // Check for spawn calls
/// let has_spawn = contains_expr(&expr, |e| matches!(e, Expr::Spawn { .. }));
/// ```
pub fn contains_expr<F>(expr: &Spanned<Expr>, predicate: F) -> bool
where
    F: Fn(&Expr) -> bool,
{
    struct Detector<F> {
        predicate: F,
        found: bool,
    }

    impl<F> Visitor for Detector<F>
    where
        F: Fn(&Expr) -> bool,
    {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if (self.predicate)(&expr.node) {
                self.found = true;
                return; // Short-circuit
            }
            walk_expr(self, expr);
        }
    }

    let mut detector = Detector {
        predicate,
        found: false,
    };
    detector.visit_expr(expr);
    detector.found
}

/// Check if a statement tree contains any statement matching a predicate.
///
/// # Examples
///
/// ```
/// // Check for yield statements
/// let has_yield = contains_stmt(&stmt, |s| matches!(s, Stmt::Yield { .. }));
/// ```
pub fn contains_stmt<F>(stmt: &Spanned<Stmt>, predicate: F) -> bool
where
    F: Fn(&Stmt) -> bool,
{
    struct Detector<F> {
        predicate: F,
        found: bool,
    }

    impl<F> Visitor for Detector<F>
    where
        F: Fn(&Stmt) -> bool,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if (self.predicate)(&stmt.node) {
                self.found = true;
                return; // Short-circuit
            }
            walk_stmt(self, stmt);
        }
    }

    let mut detector = Detector {
        predicate,
        found: false,
    };
    detector.visit_stmt(stmt);
    detector.found
}

/// Check if a block contains any statement matching a predicate.
pub fn contains_stmt_in_block<F>(block: &Spanned<Block>, predicate: F) -> bool
where
    F: Fn(&Stmt) -> bool,
{
    struct Detector<F> {
        predicate: F,
        found: bool,
    }

    impl<F> Visitor for Detector<F>
    where
        F: Fn(&Stmt) -> bool,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if (self.predicate)(&stmt.node) {
                self.found = true;
                return;
            }
            walk_stmt(self, stmt);
        }
    }

    let mut detector = Detector {
        predicate,
        found: false,
    };
    detector.visit_block(block);
    detector.found
}

// ============================================================================
// Counting Helpers
// ============================================================================

/// Count expressions matching a predicate in an expression tree.
///
/// # Examples
///
/// ```
/// // Count all method calls
/// let method_count = count_exprs(&expr, |e| matches!(e, Expr::MethodCall { .. }));
/// ```
pub fn count_exprs<F>(expr: &Spanned<Expr>, predicate: F) -> usize
where
    F: Fn(&Expr) -> bool,
{
    struct Counter<F> {
        predicate: F,
        count: usize,
    }

    impl<F> Visitor for Counter<F>
    where
        F: Fn(&Expr) -> bool,
    {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if (self.predicate)(&expr.node) {
                self.count += 1;
            }
            walk_expr(self, expr);
        }
    }

    let mut counter = Counter { predicate, count: 0 };
    counter.visit_expr(expr);
    counter.count
}

/// Count statements matching a predicate in a statement tree.
///
/// # Examples
///
/// ```
/// // Count all return statements
/// let return_count = count_stmts(&stmt, |s| matches!(s, Stmt::Return(_)));
/// ```
pub fn count_stmts<F>(stmt: &Spanned<Stmt>, predicate: F) -> usize
where
    F: Fn(&Stmt) -> bool,
{
    struct Counter<F> {
        predicate: F,
        count: usize,
    }

    impl<F> Visitor for Counter<F>
    where
        F: Fn(&Stmt) -> bool,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if (self.predicate)(&stmt.node) {
                self.count += 1;
            }
            walk_stmt(self, stmt);
        }
    }

    let mut counter = Counter { predicate, count: 0 };
    counter.visit_stmt(stmt);
    counter.count
}

/// Count statements matching a predicate in a block.
pub fn count_stmts_in_block<F>(block: &Spanned<Block>, predicate: F) -> usize
where
    F: Fn(&Stmt) -> bool,
{
    struct Counter<F> {
        predicate: F,
        count: usize,
    }

    impl<F> Visitor for Counter<F>
    where
        F: Fn(&Stmt) -> bool,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if (self.predicate)(&stmt.node) {
                self.count += 1;
            }
            walk_stmt(self, stmt);
        }
    }

    let mut counter = Counter { predicate, count: 0 };
    counter.visit_block(block);
    counter.count
}

/// Count statements matching a predicate in a function body.
pub fn count_stmts_in_function<F>(func: &Spanned<Function>, predicate: F) -> usize
where
    F: Fn(&Stmt) -> bool,
{
    struct Counter<F> {
        predicate: F,
        count: usize,
    }

    impl<F> Visitor for Counter<F>
    where
        F: Fn(&Stmt) -> bool,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if (self.predicate)(&stmt.node) {
                self.count += 1;
            }
            walk_stmt(self, stmt);
        }
    }

    let mut counter = Counter { predicate, count: 0 };
    counter.visit_function(func);
    counter.count
}

// ============================================================================
// Collection Helpers
// ============================================================================

/// Collect all expressions matching a mapper function.
///
/// The mapper returns `Some(value)` to collect a value, or `None` to skip.
///
/// # Examples
///
/// ```
/// // Collect all identifier names
/// let idents = collect_exprs(&expr, |e| {
///     if let Expr::Ident(name) = e {
///         Some(name.clone())
///     } else {
///         None
///     }
/// });
///
/// // Collect all string literals
/// let strings = collect_exprs(&expr, |e| {
///     if let Expr::StringLit(s) = e {
///         Some(s.clone())
///     } else {
///         None
///     }
/// });
/// ```
pub fn collect_exprs<F, T>(expr: &Spanned<Expr>, mapper: F) -> Vec<T>
where
    F: Fn(&Expr) -> Option<T>,
{
    struct Collector<F, T> {
        mapper: F,
        items: Vec<T>,
    }

    impl<F, T> Visitor for Collector<F, T>
    where
        F: Fn(&Expr) -> Option<T>,
    {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if let Some(item) = (self.mapper)(&expr.node) {
                self.items.push(item);
            }
            walk_expr(self, expr);
        }
    }

    let mut collector = Collector {
        mapper,
        items: Vec::new(),
    };
    collector.visit_expr(expr);
    collector.items
}

/// Collect all statements matching a mapper function.
pub fn collect_stmts<F, T>(stmt: &Spanned<Stmt>, mapper: F) -> Vec<T>
where
    F: Fn(&Stmt) -> Option<T>,
{
    struct Collector<F, T> {
        mapper: F,
        items: Vec<T>,
    }

    impl<F, T> Visitor for Collector<F, T>
    where
        F: Fn(&Stmt) -> Option<T>,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if let Some(item) = (self.mapper)(&stmt.node) {
                self.items.push(item);
            }
            walk_stmt(self, stmt);
        }
    }

    let mut collector = Collector {
        mapper,
        items: Vec::new(),
    };
    collector.visit_stmt(stmt);
    collector.items
}

/// Collect unique expressions matching a mapper function (deduplicates via HashSet).
///
/// # Examples
///
/// ```
/// // Collect unique identifier names
/// let unique_idents = collect_exprs_unique(&expr, |e| {
///     if let Expr::Ident(name) = e {
///         Some(name.clone())
///     } else {
///         None
///     }
/// });
/// ```
pub fn collect_exprs_unique<F, T>(expr: &Spanned<Expr>, mapper: F) -> Vec<T>
where
    F: Fn(&Expr) -> Option<T>,
    T: Eq + std::hash::Hash + Clone,
{
    struct Collector<F, T> {
        mapper: F,
        seen: HashSet<T>,
        items: Vec<T>,
    }

    impl<F, T> Visitor for Collector<F, T>
    where
        F: Fn(&Expr) -> Option<T>,
        T: Eq + std::hash::Hash + Clone,
    {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if let Some(item) = (self.mapper)(&expr.node) {
                if self.seen.insert(item.clone()) {
                    self.items.push(item);
                }
            }
            walk_expr(self, expr);
        }
    }

    let mut collector = Collector {
        mapper,
        seen: HashSet::new(),
        items: Vec::new(),
    };
    collector.visit_expr(expr);
    collector.items
}

/// Collect unique statements matching a mapper function.
pub fn collect_stmts_unique<F, T>(stmt: &Spanned<Stmt>, mapper: F) -> Vec<T>
where
    F: Fn(&Stmt) -> Option<T>,
    T: Eq + std::hash::Hash + Clone,
{
    struct Collector<F, T> {
        mapper: F,
        seen: HashSet<T>,
        items: Vec<T>,
    }

    impl<F, T> Visitor for Collector<F, T>
    where
        F: Fn(&Stmt) -> Option<T>,
        T: Eq + std::hash::Hash + Clone,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if let Some(item) = (self.mapper)(&stmt.node) {
                if self.seen.insert(item.clone()) {
                    self.items.push(item);
                }
            }
            walk_stmt(self, stmt);
        }
    }

    let mut collector = Collector {
        mapper,
        seen: HashSet::new(),
        items: Vec::new(),
    };
    collector.visit_stmt(stmt);
    collector.items
}

// ============================================================================
// Find Helpers
// ============================================================================

/// Find the first expression matching a predicate.
///
/// Returns a reference to the matching expression, or `None` if no match.
///
/// # Examples
///
/// ```
/// // Find first propagate operator
/// let propagate = find_expr(&expr, |e| matches!(e, Expr::Propagate { .. }));
/// ```
pub fn find_expr<'a, F>(expr: &'a Spanned<Expr>, predicate: F) -> Option<&'a Spanned<Expr>>
where
    F: Fn(&Expr) -> bool,
{
    struct Finder<'a, F> {
        predicate: F,
        found: Option<&'a Spanned<Expr>>,
    }

    impl<'a, F> Visitor for Finder<'a, F>
    where
        F: Fn(&Expr) -> bool,
    {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            if self.found.is_some() {
                return; // Already found, short-circuit
            }
            if (self.predicate)(&expr.node) {
                // SAFETY: We need to extend the lifetime here. The returned reference
                // is valid as long as the input expression is valid, which is
                // guaranteed by the function signature.
                self.found = Some(unsafe { &*(expr as *const Spanned<Expr>) });
                return;
            }
            walk_expr(self, expr);
        }
    }

    let mut finder = Finder {
        predicate,
        found: None,
    };
    finder.visit_expr(expr);
    finder.found
}

/// Find the first statement matching a predicate.
pub fn find_stmt<'a, F>(stmt: &'a Spanned<Stmt>, predicate: F) -> Option<&'a Spanned<Stmt>>
where
    F: Fn(&Stmt) -> bool,
{
    struct Finder<'a, F> {
        predicate: F,
        found: Option<&'a Spanned<Stmt>>,
    }

    impl<'a, F> Visitor for Finder<'a, F>
    where
        F: Fn(&Stmt) -> bool,
    {
        fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
            if self.found.is_some() {
                return;
            }
            if (self.predicate)(&stmt.node) {
                self.found = Some(unsafe { &*(stmt as *const Spanned<Stmt>) });
                return;
            }
            walk_stmt(self, stmt);
        }
    }

    let mut finder = Finder {
        predicate,
        found: None,
    };
    finder.visit_stmt(stmt);
    finder.found
}

// ============================================================================
// Quantifier Helpers (any / all)
// ============================================================================

/// Check if any expression of a specific type matches a predicate.
///
/// # Examples
///
/// ```
/// // Check if all identifiers are lowercase
/// let all_lowercase = !any_expr(&expr,
///     |e| matches!(e, Expr::Ident(name) if name.chars().any(|c| c.is_uppercase()))
/// );
/// ```
pub fn any_expr<F>(expr: &Spanned<Expr>, predicate: F) -> bool
where
    F: Fn(&Expr) -> bool,
{
    contains_expr(expr, predicate)
}

/// Check if any statement of a specific type matches a predicate.
pub fn any_stmt<F>(stmt: &Spanned<Stmt>, predicate: F) -> bool
where
    F: Fn(&Stmt) -> bool,
{
    contains_stmt(stmt, predicate)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn dummy_span() -> Span {
        Span {
            start: 0,
            end: 0,
            file_id: 0,
        }
    }

    fn dummy<T>(node: T) -> Spanned<T> {
        Spanned::new(node, dummy_span())
    }

    #[test]
    fn test_contains_expr_finds_propagate() {
        // Create: foo()!
        let expr = dummy(Expr::Propagate {
            expr: Box::new(dummy(Expr::Call {
                name: dummy("foo".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })),
        });

        let has_propagate = contains_expr(&expr, |e| matches!(e, Expr::Propagate { .. }));
        assert!(has_propagate, "Should find propagate operator");
    }

    #[test]
    fn test_contains_expr_nested() {
        // Create: a + b! (propagate is nested in binop RHS)
        let expr = dummy(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(dummy(Expr::Ident("a".to_string()))),
            rhs: Box::new(dummy(Expr::Propagate {
                expr: Box::new(dummy(Expr::Ident("b".to_string()))),
            })),
        });

        let has_propagate = contains_expr(&expr, |e| matches!(e, Expr::Propagate { .. }));
        assert!(has_propagate, "Should find nested propagate");
    }

    #[test]
    fn test_contains_expr_not_found() {
        let expr = dummy(Expr::IntLit(42));
        let has_propagate = contains_expr(&expr, |e| matches!(e, Expr::Propagate { .. }));
        assert!(!has_propagate, "Should not find propagate in int literal");
    }

    #[test]
    fn test_contains_stmt_finds_yield() {
        let stmt = dummy(Stmt::Yield {
            value: dummy(Expr::IntLit(42)),
        });

        let has_yield = contains_stmt(&stmt, |s| matches!(s, Stmt::Yield { .. }));
        assert!(has_yield, "Should find yield statement");
    }

    #[test]
    fn test_contains_stmt_in_block() {
        let block = dummy(Block {
            stmts: vec![
                dummy(Stmt::Let {
                    name: dummy("x".to_string()),
                    ty: None,
                    value: dummy(Expr::IntLit(1)),
                    is_mut: false,
                }),
                dummy(Stmt::Yield {
                    value: dummy(Expr::Ident("x".to_string())),
                }),
            ],
        });

        let has_yield = contains_stmt_in_block(&block, |s| matches!(s, Stmt::Yield { .. }));
        assert!(has_yield, "Should find yield in block");
    }

    #[test]
    fn test_count_exprs() {
        // Create: a + b + c (3 idents total)
        let expr = dummy(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(dummy(Expr::BinOp {
                op: BinOp::Add,
                lhs: Box::new(dummy(Expr::Ident("a".to_string()))),
                rhs: Box::new(dummy(Expr::Ident("b".to_string()))),
            })),
            rhs: Box::new(dummy(Expr::Ident("c".to_string()))),
        });

        let count = count_exprs(&expr, |e| matches!(e, Expr::Ident(_)));
        assert_eq!(count, 3, "Should count 3 identifiers");
    }

    #[test]
    fn test_count_stmts_in_block() {
        let block = dummy(Block {
            stmts: vec![
                dummy(Stmt::Return(Some(dummy(Expr::IntLit(1))))),
                dummy(Stmt::Return(Some(dummy(Expr::IntLit(2))))),
                dummy(Stmt::Let {
                    name: dummy("x".to_string()),
                    ty: None,
                    value: dummy(Expr::IntLit(3)),
                    is_mut: false,
                }),
            ],
        });

        let count = count_stmts_in_block(&block, |s| matches!(s, Stmt::Return(_)));
        assert_eq!(count, 2, "Should count 2 return statements");
    }

    #[test]
    fn test_collect_exprs() {
        // Create: a + b + 42 (collect ident names)
        let expr = dummy(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(dummy(Expr::BinOp {
                op: BinOp::Add,
                lhs: Box::new(dummy(Expr::Ident("a".to_string()))),
                rhs: Box::new(dummy(Expr::Ident("b".to_string()))),
            })),
            rhs: Box::new(dummy(Expr::IntLit(42))),
        });

        let idents = collect_exprs(&expr, |e| {
            if let Expr::Ident(name) = e {
                Some(name.clone())
            } else {
                None
            }
        });

        assert_eq!(idents, vec!["a", "b"], "Should collect both ident names");
    }

    #[test]
    fn test_collect_exprs_unique() {
        // Create: a + a + b (3 idents but only 2 unique)
        let expr = dummy(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(dummy(Expr::BinOp {
                op: BinOp::Add,
                lhs: Box::new(dummy(Expr::Ident("a".to_string()))),
                rhs: Box::new(dummy(Expr::Ident("a".to_string()))),
            })),
            rhs: Box::new(dummy(Expr::Ident("b".to_string()))),
        });

        let idents = collect_exprs_unique(&expr, |e| {
            if let Expr::Ident(name) = e {
                Some(name.clone())
            } else {
                None
            }
        });

        assert_eq!(idents.len(), 2, "Should deduplicate to 2 unique idents");
        assert!(idents.contains(&"a".to_string()));
        assert!(idents.contains(&"b".to_string()));
    }

    #[test]
    fn test_collect_stmts() {
        let block_stmt = dummy(Stmt::If {
            condition: dummy(Expr::BoolLit(true)),
            then_block: dummy(Block {
                stmts: vec![
                    dummy(Stmt::Let {
                        name: dummy("x".to_string()),
                        ty: None,
                        value: dummy(Expr::IntLit(1)),
                        is_mut: false,
                    }),
                    dummy(Stmt::Let {
                        name: dummy("y".to_string()),
                        ty: None,
                        value: dummy(Expr::IntLit(2)),
                        is_mut: false,
                    }),
                ],
            }),
            else_block: None,
        });

        let var_names = collect_stmts(&block_stmt, |s| {
            if let Stmt::Let { name, .. } = s {
                Some(name.node.clone())
            } else {
                None
            }
        });

        assert_eq!(var_names, vec!["x", "y"], "Should collect both variable names");
    }

    #[test]
    fn test_find_expr() {
        // Create: a + spawn { foo() } (find the spawn)
        let expr = dummy(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(dummy(Expr::Ident("a".to_string()))),
            rhs: Box::new(dummy(Expr::Spawn {
                call: Box::new(dummy(Expr::Call {
                    name: dummy("foo".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
        });

        let found = find_expr(&expr, |e| matches!(e, Expr::Spawn { .. }));
        assert!(found.is_some(), "Should find spawn expression");
        assert!(
            matches!(found.unwrap().node, Expr::Spawn { .. }),
            "Found expression should be spawn"
        );
    }

    #[test]
    fn test_find_expr_not_found() {
        let expr = dummy(Expr::IntLit(42));
        let found = find_expr(&expr, |e| matches!(e, Expr::Spawn { .. }));
        assert!(found.is_none(), "Should not find spawn in int literal");
    }

    #[test]
    fn test_find_stmt() {
        let if_stmt = dummy(Stmt::If {
            condition: dummy(Expr::BoolLit(true)),
            then_block: dummy(Block {
                stmts: vec![
                    dummy(Stmt::Let {
                        name: dummy("x".to_string()),
                        ty: None,
                        value: dummy(Expr::IntLit(1)),
                        is_mut: false,
                    }),
                    dummy(Stmt::Yield {
                        value: dummy(Expr::Ident("x".to_string())),
                    }),
                ],
            }),
            else_block: None,
        });

        let found = find_stmt(&if_stmt, |s| matches!(s, Stmt::Yield { .. }));
        assert!(found.is_some(), "Should find yield statement");
    }

    #[test]
    fn test_any_expr() {
        let expr = dummy(Expr::Propagate {
            expr: Box::new(dummy(Expr::IntLit(42))),
        });

        assert!(
            any_expr(&expr, |e| matches!(e, Expr::Propagate { .. })),
            "any_expr should work like contains_expr"
        );
    }

    #[test]
    fn test_any_stmt() {
        let stmt = dummy(Stmt::Return(Some(dummy(Expr::IntLit(42)))));

        assert!(
            any_stmt(&stmt, |s| matches!(s, Stmt::Return(_))),
            "any_stmt should work like contains_stmt"
        );
    }
}

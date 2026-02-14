//! Code coverage instrumentation for Pluto programs.
//!
//! This module handles the compile-time side of coverage:
//! - Scanning the AST to assign coverage point IDs
//! - Mapping points to source locations (line, column)
//! - Serializing the coverage map to JSON
//! - Reading binary counter data and generating reports
//!
//! Coverage is keyed by `span.start` byte offset — the scanner and codegen
//! both use this value to match points, avoiding iteration-order coupling.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json;

use crate::parser::ast::*;
use crate::span::{Span, Spanned};

// ── Coverage point data model ────────────────────────────────────────────────

/// The kind of code construct being instrumented.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoverageKind {
    Statement,
    FunctionEntry,
    // Branch coverage: if/else
    BranchThen,
    BranchElse,
    // Branch coverage: match arms
    MatchArm { index: u32 },
    // Branch coverage: loop body entry
    LoopEntry,
    // Branch coverage: null propagation (?)
    NullPropNull,
    NullPropValue,
    // Branch coverage: error propagation (!)
    ErrorPropError,
    ErrorPropSuccess,
}

impl CoverageKind {
    /// Whether this kind represents a branch point.
    pub fn is_branch(&self) -> bool {
        !matches!(self, CoverageKind::Statement | CoverageKind::FunctionEntry)
    }
}

/// A single instrumented point in the source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoveragePoint {
    pub id: u32,
    pub file_id: u32,
    pub byte_offset: usize,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub kind: CoverageKind,
    pub function_name: String,
    /// Discriminator for multiple coverage points at the same byte offset.
    /// 0 = primary (statement/function entry), 1+ = branch variants.
    #[serde(default)]
    pub branch_id: u32,
}

/// Metadata about a source file in the coverage map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageFile {
    pub id: u32,
    pub path: String,
}

/// The complete coverage map — static metadata produced at compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageMap {
    pub points: Vec<CoveragePoint>,
    pub files: Vec<CoverageFile>,
}

impl CoverageMap {
    /// Write the coverage map as JSON to a file.
    pub fn write_json(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Read a coverage map from a JSON file.
    pub fn read_json(path: &Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    pub fn num_points(&self) -> u32 {
        self.points.len() as u32
    }

    /// Build a lookup table: (byte_offset, branch_id) → point_id.
    /// Used by codegen to quickly find the point ID for a given statement span.
    /// branch_id 0 = primary (statement/function entry), 1+ = branch variants.
    pub fn build_span_lookup(&self) -> HashMap<(usize, u32), u32> {
        let mut lookup = HashMap::new();
        for point in &self.points {
            lookup.insert((point.byte_offset, point.branch_id), point.id);
        }
        lookup
    }
}

// ── Coverage data (runtime counters) ────────────────────────────────────────

/// Runtime counter data read from the binary file.
#[derive(Debug)]
pub struct CoverageData {
    pub counters: Vec<i64>,
}

impl CoverageData {
    /// Read binary counter data from a file.
    /// Format: [num_points: i64][counter_0: i64]...[counter_N-1: i64]
    pub fn read_binary(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        if bytes.len() < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "coverage data too short",
            ));
        }
        let num_points = i64::from_le_bytes(bytes[0..8].try_into().unwrap());
        if num_points < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "negative point count",
            ));
        }
        let expected_len = 8 + (num_points as usize) * 8;
        if bytes.len() < expected_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("expected {} bytes, got {}", expected_len, bytes.len()),
            ));
        }
        let mut counters = Vec::with_capacity(num_points as usize);
        for i in 0..num_points as usize {
            let offset = 8 + i * 8;
            let val = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            counters.push(val);
        }
        Ok(CoverageData { counters })
    }
}

// ── Line info helper ────────────────────────────────────────────────────────

/// Precomputed line start offsets for fast byte-offset → line/column conversion.
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                starts.push(i + 1);
            }
        }
        Self { line_starts: starts }
    }

    /// Convert a byte offset to a (line, column) pair (1-based).
    pub fn line_col(&self, offset: usize) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let col = offset.saturating_sub(self.line_starts[line]);
        ((line + 1) as u32, (col + 1) as u32)
    }
}

// ── AST scanner ─────────────────────────────────────────────────────────────

/// Scans a Program AST and produces a CoverageMap with one point per statement
/// and one per function entry.
pub fn build_coverage_map(
    program: &Program,
    source: &str,
    source_file: &str,
) -> CoverageMap {
    let line_index = LineIndex::new(source);
    let mut scanner = CoverageScanner {
        points: Vec::new(),
        line_index: &line_index,
        current_function: String::new(),
        file_id: 0,
    };

    let file = CoverageFile {
        id: 0,
        path: source_file.to_string(),
    };

    // Scan top-level functions
    for func in &program.functions {
        scanner.scan_function(&func.node);
    }

    // Scan class methods
    for class in &program.classes {
        for method in &class.node.methods {
            let mangled = format!("{}.{}", class.node.name.node, method.node.name.node);
            scanner.scan_function_with_name(&method.node, &mangled);
        }
    }

    // Scan app methods
    if let Some(app) = &program.app {
        for method in &app.node.methods {
            let mangled = format!("{}.{}", app.node.name.node, method.node.name.node);
            scanner.scan_function_with_name(&method.node, &mangled);
        }
    }

    // Scan stage methods
    for stage in &program.stages {
        for method in &stage.node.methods {
            let mangled = format!("{}.{}", stage.node.name.node, method.node.name.node);
            scanner.scan_function_with_name(&method.node, &mangled);
        }
    }

    CoverageMap {
        points: scanner.points,
        files: vec![file],
    }
}

struct CoverageScanner<'a> {
    points: Vec<CoveragePoint>,
    line_index: &'a LineIndex,
    current_function: String,
    file_id: u32,
}

impl<'a> CoverageScanner<'a> {
    fn scan_function(&mut self, func: &Function) {
        self.scan_function_with_name(func, &func.name.node);
    }

    fn scan_function_with_name(&mut self, func: &Function, name: &str) {
        self.current_function = name.to_string();

        // Skip functions with synthetic spans (monomorphized, closure-lifted, etc.)
        // that have offsets >= 10_000_000 (indicating they're not original source)
        if !func.body.node.stmts.is_empty() {
            let first_span = func.body.node.stmts[0].span;
            if first_span.start >= 10_000_000 {
                return;
            }
        }

        // Function entry point
        let func_span = func.name.span;
        if func_span.start < 10_000_000 {
            self.add_point(func_span, CoverageKind::FunctionEntry);
        }

        // Scan statements
        self.scan_block(&func.body.node);
    }

    fn scan_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.scan_stmt(stmt);
        }
    }

    fn scan_stmt(&mut self, stmt: &Spanned<Stmt>) {
        // Skip synthetic/offset spans
        if stmt.span.start >= 10_000_000 {
            return;
        }

        // Add a statement coverage point
        self.add_point(stmt.span, CoverageKind::Statement);

        // Recurse into nested blocks + add branch coverage points
        match &stmt.node {
            Stmt::If { condition, then_block, else_block } => {
                // Branch coverage: then path (branch_id 1, keyed by then_block span)
                if then_block.span.start < 10_000_000 {
                    self.add_point_with_branch(then_block.span, CoverageKind::BranchThen, 1);
                }
                self.scan_block(&then_block.node);
                if let Some(eb) = else_block {
                    // Branch coverage: else path (branch_id 1, keyed by else_block span)
                    if eb.span.start < 10_000_000 {
                        self.add_point_with_branch(eb.span, CoverageKind::BranchElse, 1);
                    }
                    self.scan_block(&eb.node);
                } else {
                    // Implicit else: keyed by condition span with branch_id 2
                    if condition.span.start < 10_000_000 {
                        self.add_point_with_branch(condition.span, CoverageKind::BranchElse, 2);
                    }
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                // Branch coverage: loop body entry (branch_id 1, keyed by body span)
                if body.span.start < 10_000_000 {
                    self.add_point_with_branch(body.span, CoverageKind::LoopEntry, 1);
                }
                self.scan_block(&body.node);
            }
            Stmt::Match { arms, .. } => {
                for (i, arm) in arms.iter().enumerate() {
                    // Branch coverage: match arm (branch_id 1, keyed by arm body span)
                    if arm.body.span.start < 10_000_000 {
                        self.add_point_with_branch(
                            arm.body.span,
                            CoverageKind::MatchArm { index: i as u32 },
                            1,
                        );
                    }
                    self.scan_block(&arm.body.node);
                }
            }
            Stmt::Select { arms, default, .. } => {
                for arm in arms {
                    self.scan_block(&arm.body.node);
                }
                if let Some(def) = default {
                    self.scan_block(&def.node);
                }
            }
            Stmt::Scope { body, .. } => {
                self.scan_block(&body.node);
            }
            // Leaf statements — already counted above
            Stmt::Let { .. }
            | Stmt::LetChan { .. }
            | Stmt::Assign { .. }
            | Stmt::FieldAssign { .. }
            | Stmt::IndexAssign { .. }
            | Stmt::Raise { .. }
            | Stmt::Return(_)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::Yield { .. }
            | Stmt::Expr(_) => {}
        }

        // Scan expressions in this statement for expression-level branch points
        self.scan_stmt_exprs(&stmt.node);
    }

    fn add_point(&mut self, span: Span, kind: CoverageKind) {
        self.add_point_with_branch(span, kind, 0);
    }

    fn add_point_with_branch(&mut self, span: Span, kind: CoverageKind, branch_id: u32) {
        let (line, column) = self.line_index.line_col(span.start);
        let (end_line, end_column) = self.line_index.line_col(span.end);
        self.points.push(CoveragePoint {
            id: self.points.len() as u32,
            file_id: self.file_id,
            byte_offset: span.start,
            line,
            column,
            end_line,
            end_column,
            kind,
            function_name: self.current_function.clone(),
            branch_id,
        });
    }

    /// Scan a statement's expressions for expression-level branch points (?, !).
    fn scan_stmt_exprs(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { value, .. } => {
                self.scan_expr(&value.node);
            }
            Stmt::Assign { value, .. } => self.scan_expr(&value.node),
            Stmt::FieldAssign { value, object, .. } => {
                self.scan_expr(&value.node);
                self.scan_expr(&object.node);
            }
            Stmt::IndexAssign { object, index, value, .. } => {
                self.scan_expr(&object.node);
                self.scan_expr(&index.node);
                self.scan_expr(&value.node);
            }
            Stmt::Return(Some(expr)) => self.scan_expr(&expr.node),
            Stmt::Expr(expr) => self.scan_expr(&expr.node),
            Stmt::If { condition, .. } => {
                self.scan_expr(&condition.node);
                // then/else blocks already recursed via scan_block
            }
            Stmt::While { condition, .. } => self.scan_expr(&condition.node),
            Stmt::For { iterable, .. } => self.scan_expr(&iterable.node),
            Stmt::Match { expr, .. } => self.scan_expr(&expr.node),
            Stmt::Raise { fields, .. } => {
                for (_, val) in fields {
                    self.scan_expr(&val.node);
                }
            }
            Stmt::Yield { value, .. } => self.scan_expr(&value.node),
            Stmt::Return(None)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::LetChan { .. }
            | Stmt::Scope { .. }
            | Stmt::Select { .. } => {}
        }
    }

    /// Recursively scan an expression for NullPropagate and Propagate nodes.
    fn scan_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::NullPropagate { expr: inner } => {
                if inner.span.start < 10_000_000 {
                    // branch_id 1: null path, branch_id 2: value path
                    self.add_point_with_branch(inner.span, CoverageKind::NullPropNull, 1);
                    self.add_point_with_branch(inner.span, CoverageKind::NullPropValue, 2);
                }
                self.scan_expr(&inner.node);
            }
            Expr::Propagate { expr: inner } => {
                if inner.span.start < 10_000_000 {
                    // branch_id 1: error path, branch_id 2: success path
                    self.add_point_with_branch(inner.span, CoverageKind::ErrorPropError, 1);
                    self.add_point_with_branch(inner.span, CoverageKind::ErrorPropSuccess, 2);
                }
                self.scan_expr(&inner.node);
            }
            // If-expression: add branch points for then/else paths
            Expr::If { condition, then_block, else_block } => {
                self.scan_expr(&condition.node);
                if then_block.span.start < 10_000_000 {
                    self.add_point_with_branch(then_block.span, CoverageKind::BranchThen, 1);
                }
                if else_block.span.start < 10_000_000 {
                    self.add_point_with_branch(else_block.span, CoverageKind::BranchElse, 1);
                }
                for s in &then_block.node.stmts {
                    self.scan_stmt(s);
                }
                for s in &else_block.node.stmts {
                    self.scan_stmt(s);
                }
            }
            // Match-expression: add branch points for each arm
            Expr::Match { expr: scrutinee, arms } => {
                self.scan_expr(&scrutinee.node);
                for (i, arm) in arms.iter().enumerate() {
                    if arm.value.span.start < 10_000_000 {
                        self.add_point_with_branch(
                            arm.value.span,
                            CoverageKind::MatchArm { index: i as u32 },
                            1,
                        );
                    }
                    self.scan_expr(&arm.value.node);
                }
            }
            // Recurse into sub-expressions
            Expr::BinOp { lhs, rhs, .. } => {
                self.scan_expr(&lhs.node);
                self.scan_expr(&rhs.node);
            }
            Expr::UnaryOp { operand, .. } => self.scan_expr(&operand.node),
            Expr::Call { args, .. } => {
                for arg in args {
                    self.scan_expr(&arg.node);
                }
            }
            Expr::MethodCall { object, args, .. } => {
                self.scan_expr(&object.node);
                for arg in args {
                    self.scan_expr(&arg.node);
                }
            }
            Expr::FieldAccess { object, .. } => self.scan_expr(&object.node),
            Expr::Index { object, index, .. } => {
                self.scan_expr(&object.node);
                self.scan_expr(&index.node);
            }
            Expr::ArrayLit { elements } => {
                for elem in elements {
                    self.scan_expr(&elem.node);
                }
            }
            Expr::Catch { expr: inner, .. } => self.scan_expr(&inner.node),
            Expr::Cast { expr: inner, .. } => self.scan_expr(&inner.node),
            Expr::Range { start, end, .. } => {
                self.scan_expr(&start.node);
                self.scan_expr(&end.node);
            }
            Expr::Closure { body, .. } => {
                for s in &body.node.stmts {
                    self.scan_stmt(s);
                }
            }
            Expr::Spawn { call } => self.scan_expr(&call.node),
            Expr::StringInterp { parts } => {
                for part in parts {
                    if let crate::parser::ast::StringInterpPart::Expr(expr) = part {
                        self.scan_expr(&expr.node);
                    }
                }
            }
            Expr::EnumData { fields, .. } | Expr::StructLit { fields, .. } => {
                for (_, val) in fields {
                    self.scan_expr(&val.node);
                }
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries {
                    self.scan_expr(&k.node);
                    self.scan_expr(&v.node);
                }
            }
            Expr::SetLit { elements, .. } => {
                for elem in elements {
                    self.scan_expr(&elem.node);
                }
            }
            Expr::StaticTraitCall { args, .. } => {
                for arg in args {
                    self.scan_expr(&arg.node);
                }
            }
            // Leaf expressions — no sub-expressions to scan
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::Ident(_)
            | Expr::NoneLit
            | Expr::ClosureCreate { .. }
            | Expr::EnumUnit { .. }
            | Expr::QualifiedAccess { .. } => {}
        }
    }
}

// ── Report generation ───────────────────────────────────────────────────────

/// Per-file coverage statistics.
#[derive(Debug)]
pub struct FileCoverage {
    pub path: String,
    pub total_lines: u32,
    pub covered_lines: u32,
    pub total_functions: u32,
    pub covered_functions: u32,
    pub total_branches: u32,
    pub covered_branches: u32,
}

/// Generate a terminal summary report from coverage map + data.
pub fn generate_terminal_report(
    map: &CoverageMap,
    data: &CoverageData,
) -> Vec<FileCoverage> {
    let mut file_stats: HashMap<u32, FileCoverage> = HashMap::new();

    // Initialize file entries
    for file in &map.files {
        file_stats.insert(file.id, FileCoverage {
            path: file.path.clone(),
            total_lines: 0,
            covered_lines: 0,
            total_functions: 0,
            covered_functions: 0,
            total_branches: 0,
            covered_branches: 0,
        });
    }

    // Track unique lines per file
    let mut file_lines: HashMap<u32, HashMap<u32, bool>> = HashMap::new();
    let mut file_functions: HashMap<u32, HashMap<String, bool>> = HashMap::new();
    // Track branches per file: each branch point is identified by (point_id)
    let mut file_branches: HashMap<u32, (u32, u32)> = HashMap::new(); // file_id → (total, covered)

    for point in &map.points {
        let hit = data.counters.get(point.id as usize).copied().unwrap_or(0) > 0;

        if point.kind.is_branch() {
            let (total, covered) = file_branches.entry(point.file_id).or_insert((0, 0));
            *total += 1;
            if hit {
                *covered += 1;
            }
        }

        match &point.kind {
            CoverageKind::Statement => {
                let lines = file_lines.entry(point.file_id).or_default();
                let entry = lines.entry(point.line).or_insert(false);
                if hit {
                    *entry = true;
                }
            }
            CoverageKind::FunctionEntry => {
                let funcs = file_functions.entry(point.file_id).or_default();
                let entry = funcs.entry(point.function_name.clone()).or_insert(false);
                if hit {
                    *entry = true;
                }
            }
            _ => {} // Branch kinds tracked above
        }
    }

    // Aggregate stats
    for (file_id, lines) in &file_lines {
        if let Some(stats) = file_stats.get_mut(file_id) {
            stats.total_lines = lines.len() as u32;
            stats.covered_lines = lines.values().filter(|&&v| v).count() as u32;
        }
    }
    for (file_id, funcs) in &file_functions {
        if let Some(stats) = file_stats.get_mut(file_id) {
            stats.total_functions = funcs.len() as u32;
            stats.covered_functions = funcs.values().filter(|&&v| v).count() as u32;
        }
    }
    for (file_id, (total, covered)) in &file_branches {
        if let Some(stats) = file_stats.get_mut(file_id) {
            stats.total_branches = *total;
            stats.covered_branches = *covered;
        }
    }

    let mut result: Vec<FileCoverage> = file_stats.into_values().collect();
    result.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

/// Format and print the terminal coverage summary.
pub fn print_terminal_summary(stats: &[FileCoverage]) {
    eprintln!();
    eprintln!("Coverage:");

    let mut total_lines = 0u32;
    let mut total_covered = 0u32;
    let mut total_funcs = 0u32;
    let mut total_funcs_covered = 0u32;
    let mut total_branches = 0u32;
    let mut total_branches_covered = 0u32;

    for file in stats {
        let line_pct = if file.total_lines > 0 {
            (file.covered_lines as f64 / file.total_lines as f64) * 100.0
        } else {
            100.0
        };
        if file.total_branches > 0 {
            let branch_pct = (file.covered_branches as f64 / file.total_branches as f64) * 100.0;
            eprintln!(
                "  {:<40} {:>5.1}%  ({}/{} lines, {}/{} branches [{:.1}%])",
                file.path, line_pct, file.covered_lines, file.total_lines,
                file.covered_branches, file.total_branches, branch_pct,
            );
        } else {
            eprintln!(
                "  {:<40} {:>5.1}%  ({}/{} lines)",
                file.path, line_pct, file.covered_lines, file.total_lines,
            );
        }
        total_lines += file.total_lines;
        total_covered += file.covered_lines;
        total_funcs += file.total_functions;
        total_funcs_covered += file.covered_functions;
        total_branches += file.total_branches;
        total_branches_covered += file.covered_branches;
    }

    if !stats.is_empty() {
        let total_pct = if total_lines > 0 {
            (total_covered as f64 / total_lines as f64) * 100.0
        } else {
            100.0
        };
        if total_branches > 0 {
            let branch_pct = (total_branches_covered as f64 / total_branches as f64) * 100.0;
            eprintln!(
                "  {:<40} {:>5.1}%  ({}/{} lines, {}/{} functions, {}/{} branches [{:.1}%])",
                "Total", total_pct, total_covered, total_lines,
                total_funcs_covered, total_funcs,
                total_branches_covered, total_branches, branch_pct,
            );
        } else {
            eprintln!(
                "  {:<40} {:>5.1}%  ({}/{} lines, {}/{} functions)",
                "Total", total_pct, total_covered, total_lines,
                total_funcs_covered, total_funcs,
            );
        }
    }

    eprintln!();
    eprintln!("Coverage data written to .pluto-coverage/");
}

// ── LCOV format output ─────────────────────────────────────────────────────

/// Generate LCOV-format coverage output.
///
/// Format spec: https://ltp.sourceforge.net/coverage/lcov/geninfo.1.php
///
/// Records per file:
///   TN: (test name, blank)
///   SF:<path>
///   FN:<line>,<name>
///   FNDA:<count>,<name>
///   FNF:<total_functions>
///   FNH:<hit_functions>
///   DA:<line>,<count>
///   LF:<total_lines>
///   LH:<hit_lines>
///   BRDA:<line>,<block>,<branch>,<count>
///   BRF:<total_branches>
///   BRH:<hit_branches>
///   end_of_record
pub fn generate_lcov(map: &CoverageMap, data: &CoverageData) -> String {
    let mut output = String::new();

    // Group points by file
    let mut file_points: HashMap<u32, Vec<&CoveragePoint>> = HashMap::new();
    for point in &map.points {
        file_points.entry(point.file_id).or_default().push(point);
    }

    for file in &map.files {
        let points = match file_points.get(&file.id) {
            Some(pts) => pts,
            None => continue,
        };

        output.push_str("TN:\n");
        output.push_str(&format!("SF:{}\n", file.path));

        // Function records (FN + FNDA)
        let mut functions: Vec<(&str, u32, i64)> = Vec::new(); // (name, line, count)
        let mut seen_funcs: HashMap<&str, usize> = HashMap::new();
        for point in points {
            if point.kind == CoverageKind::FunctionEntry {
                let count = data.counters.get(point.id as usize).copied().unwrap_or(0);
                if let Some(&idx) = seen_funcs.get(point.function_name.as_str()) {
                    // Aggregate (monomorphized variants)
                    functions[idx].2 += count;
                } else {
                    seen_funcs.insert(&point.function_name, functions.len());
                    functions.push((&point.function_name, point.line, count));
                }
            }
        }

        for (name, line, _) in &functions {
            output.push_str(&format!("FN:{},{}\n", line, name));
        }
        for (name, _, count) in &functions {
            output.push_str(&format!("FNDA:{},{}\n", count, name));
        }
        let fnf = functions.len();
        let fnh = functions.iter().filter(|(_, _, c)| *c > 0).count();
        output.push_str(&format!("FNF:{}\n", fnf));
        output.push_str(&format!("FNH:{}\n", fnh));

        // Line records (DA) — aggregate by line number
        let mut line_hits: HashMap<u32, i64> = HashMap::new();
        for point in points {
            if point.kind == CoverageKind::Statement {
                let count = data.counters.get(point.id as usize).copied().unwrap_or(0);
                let entry = line_hits.entry(point.line).or_insert(0);
                *entry += count;
            }
        }
        let mut lines: Vec<_> = line_hits.into_iter().collect();
        lines.sort_by_key(|(line, _)| *line);
        for (line, count) in &lines {
            output.push_str(&format!("DA:{},{}\n", line, count));
        }
        let lf = lines.len();
        let lh = lines.iter().filter(|(_, c)| *c > 0).count();
        output.push_str(&format!("LF:{}\n", lf));
        output.push_str(&format!("LH:{}\n", lh));

        // Branch records (BRDA) — each branch point gets a record
        let mut branch_block = 0u32;
        let mut branches: Vec<(u32, u32, u32, i64)> = Vec::new(); // (line, block, branch_idx, count)
        for point in points {
            if point.kind.is_branch() {
                let count = data.counters.get(point.id as usize).copied().unwrap_or(0);
                branches.push((point.line, branch_block, point.branch_id, count));
                // Increment block ID when we see a new source location
                if point.branch_id <= 1 {
                    branch_block += 1;
                }
            }
        }
        for (line, block, branch, count) in &branches {
            if *count > 0 {
                output.push_str(&format!("BRDA:{},{},{},{}\n", line, block, branch, count));
            } else {
                output.push_str(&format!("BRDA:{},{},{},-\n", line, block, branch));
            }
        }
        let brf = branches.len();
        let brh = branches.iter().filter(|(_, _, _, c)| *c > 0).count();
        output.push_str(&format!("BRF:{}\n", brf));
        output.push_str(&format!("BRH:{}\n", brh));

        output.push_str("end_of_record\n");
    }

    output
}

// ── JSON format output ─────────────────────────────────────────────────────

/// Structured JSON coverage report for programmatic consumption.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonCoverageReport {
    pub summary: JsonCoverageSummary,
    pub files: Vec<JsonFileCoverage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonCoverageSummary {
    pub total_lines: u32,
    pub covered_lines: u32,
    pub line_percent: f64,
    pub total_functions: u32,
    pub covered_functions: u32,
    pub function_percent: f64,
    pub total_branches: u32,
    pub covered_branches: u32,
    pub branch_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonFileCoverage {
    pub path: String,
    pub lines: JsonCoverageMetric,
    pub functions: JsonCoverageMetric,
    pub branches: JsonCoverageMetric,
    pub line_details: Vec<JsonLineDetail>,
    pub function_details: Vec<JsonFunctionDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonCoverageMetric {
    pub total: u32,
    pub covered: u32,
    pub percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonLineDetail {
    pub line: u32,
    pub hit_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonFunctionDetail {
    pub name: String,
    pub line: u32,
    pub hit_count: i64,
}

/// Generate a structured JSON coverage report.
pub fn generate_json_report(map: &CoverageMap, data: &CoverageData) -> JsonCoverageReport {
    let stats = generate_terminal_report(map, data);

    let mut files = Vec::new();
    let mut file_points: HashMap<u32, Vec<&CoveragePoint>> = HashMap::new();
    for point in &map.points {
        file_points.entry(point.file_id).or_default().push(point);
    }

    for file in &map.files {
        let points = file_points.get(&file.id).map(|v| v.as_slice()).unwrap_or(&[]);
        let file_stat = stats.iter().find(|s| s.path == file.path);

        // Line details
        let mut line_hits: HashMap<u32, i64> = HashMap::new();
        for point in points {
            if point.kind == CoverageKind::Statement {
                let count = data.counters.get(point.id as usize).copied().unwrap_or(0);
                let entry = line_hits.entry(point.line).or_insert(0);
                *entry += count;
            }
        }
        let mut line_details: Vec<_> = line_hits.into_iter()
            .map(|(line, hit_count)| JsonLineDetail { line, hit_count })
            .collect();
        line_details.sort_by_key(|d| d.line);

        // Function details
        let mut func_hits: HashMap<&str, (u32, i64)> = HashMap::new();
        for point in points {
            if point.kind == CoverageKind::FunctionEntry {
                let count = data.counters.get(point.id as usize).copied().unwrap_or(0);
                let entry = func_hits.entry(&point.function_name).or_insert((point.line, 0));
                entry.1 += count;
            }
        }
        let mut function_details: Vec<_> = func_hits.into_iter()
            .map(|(name, (line, hit_count))| JsonFunctionDetail {
                name: name.to_string(), line, hit_count,
            })
            .collect();
        function_details.sort_by_key(|d| d.line);

        let (tl, cl, tf, cf, tb, cb) = match file_stat {
            Some(s) => (s.total_lines, s.covered_lines, s.total_functions,
                       s.covered_functions, s.total_branches, s.covered_branches),
            None => (0, 0, 0, 0, 0, 0),
        };

        files.push(JsonFileCoverage {
            path: file.path.clone(),
            lines: JsonCoverageMetric {
                total: tl, covered: cl,
                percent: if tl > 0 { cl as f64 / tl as f64 * 100.0 } else { 100.0 },
            },
            functions: JsonCoverageMetric {
                total: tf, covered: cf,
                percent: if tf > 0 { cf as f64 / tf as f64 * 100.0 } else { 100.0 },
            },
            branches: JsonCoverageMetric {
                total: tb, covered: cb,
                percent: if tb > 0 { cb as f64 / tb as f64 * 100.0 } else { 100.0 },
            },
            line_details,
            function_details,
        });
    }

    let (stl, scl, stf, scf, stb, scb) = files.iter().fold(
        (0u32, 0u32, 0u32, 0u32, 0u32, 0u32),
        |(tl, cl, tf, cf, tb, cb), f| {
            (tl + f.lines.total, cl + f.lines.covered,
             tf + f.functions.total, cf + f.functions.covered,
             tb + f.branches.total, cb + f.branches.covered)
        },
    );

    JsonCoverageReport {
        summary: JsonCoverageSummary {
            total_lines: stl, covered_lines: scl,
            line_percent: if stl > 0 { scl as f64 / stl as f64 * 100.0 } else { 100.0 },
            total_functions: stf, covered_functions: scf,
            function_percent: if stf > 0 { scf as f64 / stf as f64 * 100.0 } else { 100.0 },
            total_branches: stb, covered_branches: scb,
            branch_percent: if stb > 0 { scb as f64 / stb as f64 * 100.0 } else { 100.0 },
        },
        files,
    }
}

// ── HTML report output ──────────────────────────────────────────────────────

/// Combined data structure for the HTML template.
/// Extends the JSON report with embedded source code.
#[derive(Debug, Serialize)]
struct HtmlCoverageData {
    summary: JsonCoverageSummary,
    files: Vec<JsonFileCoverage>,
    sources: HashMap<String, String>,
}

/// Generate a self-contained interactive HTML coverage report.
///
/// The report is a single HTML file with embedded CSS, JS, and coverage data.
/// It includes a treemap visualization, per-file source view with line-level
/// highlighting, and a sortable function table.
pub fn generate_html_report(
    map: &CoverageMap,
    data: &CoverageData,
    source_dir: &Path,
) -> String {
    let report = generate_json_report(map, data);

    // Read source files to embed in the report
    let mut sources = HashMap::new();
    for file in &map.files {
        let file_path = source_dir.join(&file.path);
        if let Ok(contents) = std::fs::read_to_string(&file_path) {
            sources.insert(file.path.clone(), contents);
        }
    }

    let html_data = HtmlCoverageData {
        summary: report.summary,
        files: report.files,
        sources,
    };

    let json = serde_json::to_string(&html_data).unwrap_or_else(|_| "null".to_string());

    let template = include_str!("coverage_template.html");
    template.replace("/*COVERAGE_DATA*/null", &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_index_single_line() {
        let idx = LineIndex::new("hello world");
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_col(5), (1, 6));
    }

    #[test]
    fn test_line_index_multiple_lines() {
        let idx = LineIndex::new("hello\nworld\nfoo");
        assert_eq!(idx.line_col(0), (1, 1));  // 'h'
        assert_eq!(idx.line_col(6), (2, 1));  // 'w'
        assert_eq!(idx.line_col(12), (3, 1)); // 'f'
    }

    #[test]
    fn test_line_index_end_of_line() {
        let idx = LineIndex::new("abc\ndef");
        assert_eq!(idx.line_col(2), (1, 3));  // 'c'
        assert_eq!(idx.line_col(3), (1, 4));  // '\n'
        assert_eq!(idx.line_col(4), (2, 1));  // 'd'
    }

    #[test]
    fn test_coverage_kind_serialize() {
        let kind = CoverageKind::Statement;
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains("Statement"));
    }

    #[test]
    fn test_coverage_map_roundtrip() {
        let map = CoverageMap {
            points: vec![CoveragePoint {
                id: 0,
                file_id: 0,
                byte_offset: 0,
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 10,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            }],
            files: vec![CoverageFile {
                id: 0,
                path: "test.pluto".to_string(),
            }],
        };
        let json = serde_json::to_string(&map).unwrap();
        let deserialized: CoverageMap = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.points.len(), 1);
        assert_eq!(deserialized.files.len(), 1);
    }

    #[test]
    fn test_span_lookup() {
        let map = CoverageMap {
            points: vec![
                CoveragePoint {
                    id: 0, file_id: 0, byte_offset: 3,
                    line: 1, column: 1, end_line: 1, end_column: 10,
                    kind: CoverageKind::FunctionEntry,
                    function_name: "main".to_string(),
                    branch_id: 0,
                },
                CoveragePoint {
                    id: 1, file_id: 0, byte_offset: 20,
                    line: 2, column: 5, end_line: 2, end_column: 20,
                    kind: CoverageKind::Statement,
                    function_name: "main".to_string(),
                    branch_id: 0,
                },
            ],
            files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
        };
        let lookup = map.build_span_lookup();
        assert_eq!(lookup.get(&(3, 0)), Some(&0));
        assert_eq!(lookup.get(&(20, 0)), Some(&1));
    }

    #[test]
    fn test_generate_terminal_report() {
        let map = CoverageMap {
            points: vec![
                CoveragePoint {
                    id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                    end_line: 1, end_column: 10,
                    kind: CoverageKind::FunctionEntry,
                    function_name: "main".to_string(),
                    branch_id: 0,
                },
                CoveragePoint {
                    id: 1, file_id: 0, byte_offset: 10, line: 2, column: 1,
                    end_line: 2, end_column: 15,
                    kind: CoverageKind::Statement,
                    function_name: "main".to_string(),
                    branch_id: 0,
                },
                CoveragePoint {
                    id: 2, file_id: 0, byte_offset: 25, line: 3, column: 1,
                    end_line: 3, end_column: 15,
                    kind: CoverageKind::Statement,
                    function_name: "main".to_string(),
                    branch_id: 0,
                },
            ],
            files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
        };
        let data = CoverageData { counters: vec![1, 5, 0] };
        let stats = generate_terminal_report(&map, &data);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].total_lines, 2); // 2 statement lines
        assert_eq!(stats[0].covered_lines, 1); // line 2 hit, line 3 not
        assert_eq!(stats[0].total_functions, 1);
        assert_eq!(stats[0].covered_functions, 1);
    }
}

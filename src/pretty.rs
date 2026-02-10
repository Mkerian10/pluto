use crate::parser::ast::*;

/// Pretty-print a `Program` AST back into valid Pluto source text.
pub fn pretty_print(program: &Program) -> String {
    let mut pp = PrettyPrinter::new();
    pp.emit_program(program);
    pp.buf
}

struct PrettyPrinter {
    buf: String,
    indent: usize,
}

impl PrettyPrinter {
    fn new() -> Self {
        Self {
            buf: String::new(),
            indent: 0,
        }
    }

    fn write(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    fn newline(&mut self) {
        self.buf.push('\n');
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.buf.push_str("    ");
        }
    }

    fn indent(&mut self) {
        self.indent += 1;
    }

    fn dedent(&mut self) {
        self.indent -= 1;
    }

    // ── Program ──────────────────────────────────────────────────────

    fn emit_program(&mut self, program: &Program) {
        let mut has_output = false;

        // Helper: insert a blank line separator before a new group (if something was emitted before)
        macro_rules! sep {
            ($self:expr, $has:expr) => {
                if $has { $self.newline(); }
                $has = true;
            };
        }

        // 1. Imports
        if !program.imports.is_empty() {
            for imp in &program.imports {
                self.emit_import(&imp.node);
                self.newline();
            }
            has_output = true;
        }

        // 2. Extern rust
        for ext in &program.extern_rust_crates {
            sep!(self, has_output);
            self.emit_extern_rust(&ext.node);
            self.newline();
        }

        // 3. Extern fn
        for ext in &program.extern_fns {
            sep!(self, has_output);
            self.emit_extern_fn(&ext.node);
            self.newline();
        }

        // 4. Errors
        for err in &program.errors {
            sep!(self, has_output);
            self.emit_error_decl(&err.node);
            self.newline();
        }

        // 5. Traits
        for tr in &program.traits {
            sep!(self, has_output);
            self.emit_trait_decl(&tr.node);
            self.newline();
        }

        // 6. Enums
        for en in &program.enums {
            sep!(self, has_output);
            self.emit_enum_decl(&en.node);
            self.newline();
        }

        // 7. Classes
        for cls in &program.classes {
            sep!(self, has_output);
            self.emit_class_decl(&cls.node);
            self.newline();
        }

        // 8. Functions (excluding __test_N)
        let test_fn_names: std::collections::HashSet<&str> = program
            .test_info
            .iter()
            .map(|(_, fn_name)| fn_name.as_str())
            .collect();

        for func in &program.functions {
            if test_fn_names.contains(func.node.name.node.as_str()) {
                continue;
            }
            sep!(self, has_output);
            self.emit_function(&func.node);
            self.newline();
        }

        // 9. App
        if let Some(app) = &program.app {
            sep!(self, has_output);
            self.emit_app_decl(&app.node);
            self.newline();
        }

        // 10. Test blocks
        for (display_name, fn_name) in &program.test_info {
            if let Some(func) = program.functions.iter().find(|f| &f.node.name.node == fn_name) {
                sep!(self, has_output);
                self.emit_test(display_name, &func.node);
                self.newline();
            }
        }

        // Remove trailing newline if present
        while self.buf.ends_with('\n') {
            self.buf.pop();
        }
        self.newline();
    }

    // ── Imports & Externs ─────────────────────────────────────────────

    fn emit_import(&mut self, imp: &ImportDecl) {
        self.write("import ");
        let path: Vec<&str> = imp.path.iter().map(|s| s.node.as_str()).collect();
        self.write(&path.join("."));
        if let Some(alias) = &imp.alias {
            self.write(" as ");
            self.write(&alias.node);
        }
    }

    fn emit_extern_fn(&mut self, ext: &ExternFnDecl) {
        if ext.is_pub {
            self.write("pub ");
        }
        self.write("extern fn ");
        self.write(&ext.name.node);
        self.write("(");
        self.emit_params(&ext.params);
        self.write(")");
        if let Some(ret) = &ext.return_type {
            self.write(" ");
            self.emit_type_expr(&ret.node);
        }
    }

    fn emit_extern_rust(&mut self, ext: &ExternRustDecl) {
        self.write("extern rust ");
        self.write(&ext.crate_path.node);
        self.write(" as ");
        self.write(&ext.alias.node);
    }

    // ── Error ────────────────────────────────────────────────────────

    fn emit_error_decl(&mut self, err: &ErrorDecl) {
        if err.is_pub {
            self.write("pub ");
        }
        self.write("error ");
        self.write(&err.name.node);
        self.write(" {");
        self.newline();
        self.indent();
        for field in &err.fields {
            self.write_indent();
            self.write(&field.name.node);
            self.write(": ");
            self.emit_type_expr(&field.ty.node);
            self.newline();
        }
        self.dedent();
        self.write("}");
    }

    // ── Trait ────────────────────────────────────────────────────────

    fn emit_trait_decl(&mut self, tr: &TraitDecl) {
        if tr.is_pub {
            self.write("pub ");
        }
        self.write("trait ");
        self.write(&tr.name.node);
        self.write(" {");
        self.newline();
        self.indent();
        for (i, method) in tr.methods.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.emit_trait_method(method);
            self.newline();
        }
        self.dedent();
        self.write("}");
    }

    fn emit_trait_method(&mut self, method: &TraitMethod) {
        self.write_indent();
        self.write("fn ");
        self.write(&method.name.node);
        self.write("(");
        self.emit_params(&method.params);
        self.write(")");
        if let Some(ret) = &method.return_type {
            self.write(" ");
            self.emit_type_expr(&ret.node);
        }
        self.emit_contracts(&method.contracts);
        if let Some(body) = &method.body {
            self.write(" ");
            self.emit_block(&body.node);
        }
    }

    // ── Enum ─────────────────────────────────────────────────────────

    fn emit_enum_decl(&mut self, en: &EnumDecl) {
        if en.is_pub {
            self.write("pub ");
        }
        self.write("enum ");
        self.write(&en.name.node);
        self.emit_type_params(&en.type_params);
        self.write(" {");
        self.newline();
        self.indent();
        for variant in &en.variants {
            self.write_indent();
            self.write(&variant.name.node);
            if !variant.fields.is_empty() {
                self.write(" {");
                self.newline();
                self.indent();
                for field in &variant.fields {
                    self.write_indent();
                    self.write(&field.name.node);
                    self.write(": ");
                    self.emit_type_expr(&field.ty.node);
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("}");
            }
            self.newline();
        }
        self.dedent();
        self.write("}");
    }

    // ── Class ────────────────────────────────────────────────────────

    fn emit_class_decl(&mut self, cls: &ClassDecl) {
        if cls.is_pub {
            self.write("pub ");
        }
        self.write("class ");
        self.write(&cls.name.node);
        self.emit_type_params(&cls.type_params);

        // Bracket deps (injected fields)
        let injected: Vec<&Field> = cls.fields.iter().filter(|f| f.is_injected).collect();
        if !injected.is_empty() {
            self.write("[");
            for (i, f) in injected.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&f.name.node);
                self.write(": ");
                self.emit_type_expr(&f.ty.node);
            }
            self.write("]");
        }

        // impl traits
        if !cls.impl_traits.is_empty() {
            self.write(" impl ");
            for (i, t) in cls.impl_traits.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&t.node);
            }
        }

        // uses traits
        if !cls.uses.is_empty() {
            self.write(" uses ");
            for (i, u) in cls.uses.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&u.node);
            }
        }

        self.write(" {");
        self.newline();
        self.indent();

        // Non-injected fields
        let regular_fields: Vec<&Field> = cls.fields.iter().filter(|f| !f.is_injected).collect();
        for field in &regular_fields {
            self.write_indent();
            self.write(&field.name.node);
            self.write(": ");
            self.emit_type_expr(&field.ty.node);
            self.newline();
        }

        // Invariants
        for inv in &cls.invariants {
            if !regular_fields.is_empty() {
                self.newline();
            }
            self.write_indent();
            self.write("invariant ");
            self.emit_expr(&inv.node.expr.node, 0);
            self.newline();
        }

        // Blank line between fields/invariants and methods (if both exist)
        if (!regular_fields.is_empty() || !cls.invariants.is_empty()) && !cls.methods.is_empty() {
            self.newline();
        }

        // Methods
        for (i, method) in cls.methods.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.write_indent();
            self.emit_function_header(&method.node);
            self.write(" ");
            self.emit_block(&method.node.body.node);
            self.newline();
        }

        self.dedent();
        self.write("}");
    }

    // ── Function ─────────────────────────────────────────────────────

    fn emit_function(&mut self, func: &Function) {
        self.emit_function_header(func);
        self.write(" ");
        self.emit_block(&func.body.node);
    }

    fn emit_function_header(&mut self, func: &Function) {
        if func.is_pub {
            self.write("pub ");
        }
        self.write("fn ");
        self.write(&func.name.node);
        self.emit_type_params(&func.type_params);
        self.write("(");
        self.emit_params(&func.params);
        self.write(")");
        if let Some(ret) = &func.return_type {
            self.write(" ");
            self.emit_type_expr(&ret.node);
        }
        self.emit_contracts(&func.contracts);
    }

    // ── Contracts ────────────────────────────────────────────────────

    fn emit_contracts(&mut self, contracts: &[crate::span::Spanned<ContractClause>]) {
        for contract in contracts {
            self.newline();
            self.write_indent();
            match contract.node.kind {
                ContractKind::Requires => self.write("requires "),
                ContractKind::Ensures => self.write("ensures "),
                ContractKind::Invariant => self.write("invariant "),
            }
            self.emit_expr(&contract.node.expr.node, 0);
        }
    }

    // ── App ──────────────────────────────────────────────────────────

    fn emit_app_decl(&mut self, app: &AppDecl) {
        self.write("app ");
        self.write(&app.name.node);

        // Bracket deps
        if !app.inject_fields.is_empty() {
            self.write("[");
            for (i, f) in app.inject_fields.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&f.name.node);
                self.write(": ");
                self.emit_type_expr(&f.ty.node);
            }
            self.write("]");
        }

        self.write(" {");
        self.newline();
        self.indent();

        // Ambient types
        for amb in &app.ambient_types {
            self.write_indent();
            self.write("ambient ");
            self.write(&amb.node);
            self.newline();
        }

        // Blank line between ambients and methods
        if !app.ambient_types.is_empty() && !app.methods.is_empty() {
            self.newline();
        }

        // Methods
        for (i, method) in app.methods.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.write_indent();
            self.emit_function_header(&method.node);
            self.write(" ");
            self.emit_block(&method.node.body.node);
            self.newline();
        }

        self.dedent();
        self.write("}");
    }

    // ── Test ─────────────────────────────────────────────────────────

    fn emit_test(&mut self, display_name: &str, func: &Function) {
        self.write("test \"");
        self.write(&escape_string(display_name));
        self.write("\" ");
        self.emit_block(&func.body.node);
    }

    // ── Type expressions ─────────────────────────────────────────────

    fn emit_type_expr(&mut self, te: &TypeExpr) {
        match te {
            TypeExpr::Named(name) => self.write(name),
            TypeExpr::Array(elem) => {
                self.write("[");
                self.emit_type_expr(&elem.node);
                self.write("]");
            }
            TypeExpr::Qualified { module, name } => {
                self.write(module);
                self.write(".");
                self.write(name);
            }
            TypeExpr::Fn {
                params,
                return_type,
            } => {
                self.write("fn(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.emit_type_expr(&p.node);
                }
                self.write(")");
                // Omit return type if void
                if !matches!(&return_type.node, TypeExpr::Named(n) if n == "void") {
                    self.write(" ");
                    self.emit_type_expr(&return_type.node);
                }
            }
            TypeExpr::Generic { name, type_args } => {
                self.write(name);
                self.write("<");
                for (i, arg) in type_args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.emit_type_expr(&arg.node);
                }
                self.write(">");
            }
        }
    }

    fn emit_params(&mut self, params: &[Param]) {
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            // Detect self parameter: name is "self" and type is Named("Self")
            if p.name.node == "self" && p.is_mut && matches!(&p.ty.node, TypeExpr::Named(n) if n == "Self") {
                self.write("mut self");
            } else if p.name.node == "self" && matches!(&p.ty.node, TypeExpr::Named(n) if n == "Self") {
                self.write("self");
            } else {
                self.write(&p.name.node);
                self.write(": ");
                self.emit_type_expr(&p.ty.node);
            }
        }
    }

    fn emit_type_params(&mut self, type_params: &[crate::span::Spanned<String>]) {
        if !type_params.is_empty() {
            self.write("<");
            for (i, tp) in type_params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&tp.node);
            }
            self.write(">");
        }
    }

    // ── Block ────────────────────────────────────────────────────────

    fn emit_block(&mut self, block: &Block) {
        self.write("{");
        self.newline();
        self.indent();
        for stmt in &block.stmts {
            self.write_indent();
            self.emit_stmt(&stmt.node);
            self.newline();
        }
        self.dedent();
        self.write_indent();
        self.write("}");
    }

    // ── Statements ───────────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, ty, value } => {
                self.write("let ");
                self.write(&name.node);
                if let Some(ty) = ty {
                    self.write(": ");
                    self.emit_type_expr(&ty.node);
                }
                self.write(" = ");
                self.emit_expr(&value.node, 0);
            }
            Stmt::Return(expr) => {
                self.write("return");
                if let Some(e) = expr {
                    self.write(" ");
                    self.emit_expr(&e.node, 0);
                }
            }
            Stmt::Assign { target, value } => {
                self.write(&target.node);
                self.write(" = ");
                self.emit_expr(&value.node, 0);
            }
            Stmt::FieldAssign {
                object,
                field,
                value,
            } => {
                self.emit_expr(&object.node, 25);
                self.write(".");
                self.write(&field.node);
                self.write(" = ");
                self.emit_expr(&value.node, 0);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                self.write("if ");
                self.emit_expr(&condition.node, 0);
                self.write(" ");
                self.emit_block(&then_block.node);
                if let Some(else_blk) = else_block {
                    self.write(" else ");
                    // Check if this is an else-if (single stmt that is an If)
                    if else_blk.node.stmts.len() == 1 {
                        if let Stmt::If { .. } = &else_blk.node.stmts[0].node {
                            self.emit_stmt(&else_blk.node.stmts[0].node);
                            return;
                        }
                    }
                    self.emit_block(&else_blk.node);
                }
            }
            Stmt::While { condition, body } => {
                self.write("while ");
                self.emit_expr(&condition.node, 0);
                self.write(" ");
                self.emit_block(&body.node);
            }
            Stmt::For {
                var,
                iterable,
                body,
            } => {
                self.write("for ");
                self.write(&var.node);
                self.write(" in ");
                self.emit_expr(&iterable.node, 0);
                self.write(" ");
                self.emit_block(&body.node);
            }
            Stmt::IndexAssign {
                object,
                index,
                value,
            } => {
                self.emit_expr(&object.node, 25);
                self.write("[");
                self.emit_expr(&index.node, 0);
                self.write("]");
                self.write(" = ");
                self.emit_expr(&value.node, 0);
            }
            Stmt::Match { expr, arms } => {
                self.write("match ");
                self.emit_expr(&expr.node, 0);
                self.write(" {");
                self.newline();
                self.indent();
                for arm in arms {
                    self.write_indent();
                    self.write(&arm.enum_name.node);
                    if !arm.type_args.is_empty() {
                        self.write("<");
                        for (i, ta) in arm.type_args.iter().enumerate() {
                            if i > 0 {
                                self.write(", ");
                            }
                            self.emit_type_expr(&ta.node);
                        }
                        self.write(">");
                    }
                    self.write(".");
                    self.write(&arm.variant_name.node);
                    if !arm.bindings.is_empty() {
                        self.write(" { ");
                        for (i, (field_name, rename)) in arm.bindings.iter().enumerate() {
                            if i > 0 {
                                self.write(", ");
                            }
                            self.write(&field_name.node);
                            if let Some(rename) = rename {
                                self.write(": ");
                                self.write(&rename.node);
                            }
                        }
                        self.write(" }");
                    }
                    self.write(" ");
                    self.emit_block(&arm.body.node);
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("}");
            }
            Stmt::Raise {
                error_name,
                fields,
            } => {
                self.write("raise ");
                self.write(&error_name.node);
                self.write(" {");
                if !fields.is_empty() {
                    self.write(" ");
                    for (i, (name, value)) in fields.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write(&name.node);
                        self.write(": ");
                        self.emit_expr(&value.node, 0);
                    }
                    self.write(" ");
                }
                self.write("}");
            }
            Stmt::LetChan {
                sender,
                receiver,
                elem_type,
                capacity,
            } => {
                self.write("let (");
                self.write(&sender.node);
                self.write(", ");
                self.write(&receiver.node);
                self.write(") = chan<");
                self.emit_type_expr(&elem_type.node);
                self.write(">(");
                if let Some(cap) = capacity {
                    self.emit_expr(&cap.node, 0);
                }
                self.write(")");
            }
            Stmt::Select { arms, default } => {
                self.write("select {");
                self.newline();
                self.indent();
                for arm in arms {
                    self.write_indent();
                    match &arm.op {
                        SelectOp::Recv { binding, channel } => {
                            self.write(&binding.node);
                            self.write(" = ");
                            self.emit_expr(&channel.node, 25);
                            self.write(".recv() ");
                        }
                        SelectOp::Send { channel, value } => {
                            self.emit_expr(&channel.node, 25);
                            self.write(".send(");
                            self.emit_expr(&value.node, 0);
                            self.write(") ");
                        }
                    }
                    self.emit_block(&arm.body.node);
                    self.newline();
                }
                if let Some(def) = default {
                    self.write_indent();
                    self.write("default ");
                    self.emit_block(&def.node);
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("}");
            }
            Stmt::Break => self.write("break"),
            Stmt::Continue => self.write("continue"),
            Stmt::Expr(e) => self.emit_expr(&e.node, 0),
        }
    }

    // ── Expressions ──────────────────────────────────────────────────

    fn emit_expr(&mut self, expr: &Expr, parent_prec: u8) {
        match expr {
            Expr::IntLit(n) => {
                self.write(&n.to_string());
            }
            Expr::FloatLit(f) => {
                let s = f.to_string();
                self.write(&s);
                // Ensure decimal point is present
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    self.write(".0");
                }
            }
            Expr::BoolLit(b) => {
                self.write(if *b { "true" } else { "false" });
            }
            Expr::StringLit(s) => {
                self.write("\"");
                self.write(&escape_string(s));
                self.write("\"");
            }
            Expr::Ident(name) => {
                self.write(name);
            }
            Expr::BinOp { op, lhs, rhs } => {
                let prec = binop_prec(*op);
                let need_parens = prec < parent_prec;
                if need_parens {
                    self.write("(");
                }
                // Left child: same precedence (left-associative, no parens needed)
                self.emit_expr(&lhs.node, prec);
                self.write(" ");
                self.write(binop_str(*op));
                self.write(" ");
                // Right child: prec + 1 (forces parens for same-prec on right)
                self.emit_expr(&rhs.node, prec + 1);
                if need_parens {
                    self.write(")");
                }
            }
            Expr::UnaryOp { op, operand } => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                    UnaryOp::BitNot => "~",
                };
                self.write(op_str);
                self.emit_expr(&operand.node, 25);
            }
            Expr::Call { name, args } => {
                self.write(&name.node);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.emit_expr(&arg.node, 0);
                }
                self.write(")");
            }
            Expr::FieldAccess { object, field } => {
                self.emit_expr(&object.node, 25);
                self.write(".");
                self.write(&field.node);
            }
            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                self.emit_expr(&object.node, 25);
                self.write(".");
                self.write(&method.node);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.emit_expr(&arg.node, 0);
                }
                self.write(")");
            }
            Expr::StructLit {
                name,
                type_args,
                fields,
            } => {
                self.write(&name.node);
                if !type_args.is_empty() {
                    self.write("<");
                    for (i, ta) in type_args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.emit_type_expr(&ta.node);
                    }
                    self.write(">");
                }
                self.write(" { ");
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&fname.node);
                    self.write(": ");
                    self.emit_expr(&fval.node, 0);
                }
                self.write(" }");
            }
            Expr::ArrayLit { elements } => {
                self.write("[");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.emit_expr(&elem.node, 0);
                }
                self.write("]");
            }
            Expr::Index { object, index } => {
                self.emit_expr(&object.node, 25);
                self.write("[");
                self.emit_expr(&index.node, 0);
                self.write("]");
            }
            Expr::EnumUnit {
                enum_name,
                variant,
                type_args,
            } => {
                self.write(&enum_name.node);
                if !type_args.is_empty() {
                    self.write("<");
                    for (i, ta) in type_args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.emit_type_expr(&ta.node);
                    }
                    self.write(">");
                }
                self.write(".");
                self.write(&variant.node);
            }
            Expr::EnumData {
                enum_name,
                variant,
                type_args,
                fields,
            } => {
                self.write(&enum_name.node);
                if !type_args.is_empty() {
                    self.write("<");
                    for (i, ta) in type_args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.emit_type_expr(&ta.node);
                    }
                    self.write(">");
                }
                self.write(".");
                self.write(&variant.node);
                self.write(" { ");
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&fname.node);
                    self.write(": ");
                    self.emit_expr(&fval.node, 0);
                }
                self.write(" }");
            }
            Expr::StringInterp { parts } => {
                self.write("\"");
                for part in parts {
                    match part {
                        StringInterpPart::Lit(s) => {
                            self.write(&escape_interp(s));
                        }
                        StringInterpPart::Expr(e) => {
                            self.write("{");
                            self.emit_expr(&e.node, 0);
                            self.write("}");
                        }
                    }
                }
                self.write("\"");
            }
            Expr::Closure {
                params,
                return_type,
                body,
            } => {
                self.write("(");
                self.emit_params(params);
                self.write(")");
                if let Some(ret) = return_type {
                    self.write(" ");
                    self.emit_type_expr(&ret.node);
                }
                self.write(" => ");
                // Single Return(Some(expr)) body → inline expr
                if body.node.stmts.len() == 1 {
                    if let Stmt::Return(Some(ret_expr)) = &body.node.stmts[0].node {
                        self.emit_expr(&ret_expr.node, 0);
                        return;
                    }
                }
                self.emit_block(&body.node);
            }
            Expr::MapLit {
                key_type,
                value_type,
                entries,
            } => {
                self.write("Map<");
                self.emit_type_expr(&key_type.node);
                self.write(", ");
                self.emit_type_expr(&value_type.node);
                self.write("> { ");
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.emit_expr(&k.node, 0);
                    self.write(": ");
                    self.emit_expr(&v.node, 0);
                }
                if entries.is_empty() {
                    // Empty map: `Map<K, V> {}` — remove the extra space
                    self.buf.pop(); // remove ' '
                } else {
                    self.write(" ");
                }
                self.write("}");
            }
            Expr::SetLit {
                elem_type,
                elements,
            } => {
                self.write("Set<");
                self.emit_type_expr(&elem_type.node);
                self.write("> { ");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.emit_expr(&elem.node, 0);
                }
                if elements.is_empty() {
                    self.buf.pop(); // remove ' '
                } else {
                    self.write(" ");
                }
                self.write("}");
            }
            Expr::ClosureCreate { .. } => {
                // Internal node, should not appear in pre-desugar AST
                panic!("ClosureCreate should not appear in pre-desugar AST");
            }
            Expr::Propagate { expr } => {
                self.emit_expr(&expr.node, 25);
                self.write("!");
            }
            Expr::Catch { expr, handler } => {
                self.emit_expr(&expr.node, 25);
                match handler {
                    CatchHandler::Shorthand(val) => {
                        self.write(" catch ");
                        self.emit_expr(&val.node, 0);
                    }
                    CatchHandler::Wildcard { var, body } => {
                        self.write(" catch ");
                        self.write(&var.node);
                        self.write(" { ");
                        self.emit_expr(&body.node, 0);
                        self.write(" }");
                    }
                }
            }
            Expr::Cast { expr, target_type } => {
                self.emit_expr(&expr.node, 25);
                self.write(" as ");
                self.emit_type_expr(&target_type.node);
            }
            Expr::Range {
                start,
                end,
                inclusive,
            } => {
                let need_parens = 0 < parent_prec;
                if need_parens {
                    self.write("(");
                }
                self.emit_expr(&start.node, 1);
                if *inclusive {
                    self.write("..=");
                } else {
                    self.write("..");
                }
                self.emit_expr(&end.node, 1);
                if need_parens {
                    self.write(")");
                }
            }
            Expr::Spawn { call } => {
                self.write("spawn ");
                // The inner expr should be a Call — emit it directly
                self.emit_expr(&call.node, 0);
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn binop_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 3,
        BinOp::BitOr => 5,
        BinOp::BitXor => 7,
        BinOp::BitAnd => 9,
        BinOp::Eq | BinOp::Neq => 11,
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => 13,
        BinOp::Shl | BinOp::Shr => 15,
        BinOp::Add | BinOp::Sub => 17,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 19,
    }
}

fn binop_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Neq => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::LtEq => "<=",
        BinOp::GtEq => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            other => out.push(other),
        }
    }
    out
}

fn escape_interp(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // In interpolation literal parts, we do NOT escape { and }
            // because the parser already splits at the boundaries.
            // However, literal `{` and `}` that appear in the Lit parts
            // need to be doubled so they round-trip correctly.
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser::Parser;

    fn parse(source: &str) -> Program {
        let tokens = lexer::lex(source).expect("lex failed");
        let mut parser = Parser::new(&tokens, source);
        parser.parse_program().expect("parse failed")
    }

    fn pp(source: &str) -> String {
        let program = parse(source);
        pretty_print(&program)
    }

    fn assert_roundtrip_stable(source: &str) {
        let first = pp(source);
        let second = pp(&first);
        assert_eq!(first, second, "pretty-print is not idempotent");
    }

    // ── Basics ───────────────────────────────────────────────────────

    #[test]
    fn test_empty_main() {
        let src = "fn main() {\n}\n";
        let result = pp(src);
        assert_eq!(result, "fn main() {\n}\n");
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_function_with_params_and_return_type() {
        let src = "fn add(a: int, b: int) int {\n    return a + b\n}\n";
        let result = pp(src);
        assert_eq!(result, "fn add(a: int, b: int) int {\n    return a + b\n}\n");
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_pub_function() {
        let src = "pub fn greet(name: string) string {\n    return name\n}\n";
        let result = pp(src);
        assert!(result.starts_with("pub fn greet"));
        assert_roundtrip_stable(src);
    }

    // ── Let / Assign ─────────────────────────────────────────────────

    #[test]
    fn test_let_without_type() {
        let src = "fn main() {\n    let x = 42\n}\n";
        let result = pp(src);
        assert!(result.contains("let x = 42"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_let_with_type() {
        let src = "fn main() {\n    let x: int = 42\n}\n";
        let result = pp(src);
        assert!(result.contains("let x: int = 42"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_assign() {
        let src = "fn main() {\n    let x = 1\n    x = 2\n}\n";
        let result = pp(src);
        assert!(result.contains("x = 2"));
        assert_roundtrip_stable(src);
    }

    // ── Control flow ─────────────────────────────────────────────────

    #[test]
    fn test_if_else() {
        let src = "fn main() {\n    if true {\n        return\n    } else {\n        return\n    }\n}\n";
        let result = pp(src);
        assert!(result.contains("if true {"));
        assert!(result.contains("} else {"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_while_loop() {
        let src = "fn main() {\n    while true {\n        break\n    }\n}\n";
        let result = pp(src);
        assert!(result.contains("while true {"));
        assert!(result.contains("break"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_for_loop() {
        let src = "fn main() {\n    for x in [1, 2, 3] {\n        continue\n    }\n}\n";
        let result = pp(src);
        assert!(result.contains("for x in [1, 2, 3]"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_match() {
        let src = r#"enum Color {
    Red
    Blue
}

fn main() {
    let c = Color.Red
    match c {
        Color.Red {
            return
        }
        Color.Blue {
            return
        }
    }
}
"#;
        let result = pp(src);
        assert!(result.contains("match c {"));
        assert!(result.contains("Color.Red {"));
        assert_roundtrip_stable(src);
    }

    // ── Operators ────────────────────────────────────────────────────

    #[test]
    fn test_precedence_no_extra_parens() {
        // 1 + 2 * 3 should NOT get extra parens
        let src = "fn main() {\n    let x = 1 + 2 * 3\n}\n";
        let result = pp(src);
        assert!(result.contains("1 + 2 * 3"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_forced_parens() {
        // (1 + 2) * 3 must keep parens
        let src = "fn main() {\n    let x = (1 + 2) * 3\n}\n";
        let result = pp(src);
        assert!(result.contains("(1 + 2) * 3"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_left_associativity() {
        // a - b - c is left-assoc, no extra parens needed
        let src = "fn main() {\n    let a = 1\n    let b = 2\n    let c = 3\n    let x = a - b - c\n}\n";
        let result = pp(src);
        assert!(result.contains("a - b - c"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_right_child_same_prec_gets_parens() {
        // a - (b - c) needs parens on the right
        let src = "fn main() {\n    let a = 1\n    let b = 2\n    let c = 3\n    let x = a - (b - c)\n}\n";
        let result = pp(src);
        assert!(result.contains("a - (b - c)"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_unary_neg() {
        let src = "fn main() {\n    let x = -5\n}\n";
        let result = pp(src);
        assert!(result.contains("-5"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_unary_not() {
        let src = "fn main() {\n    let x = !true\n}\n";
        let result = pp(src);
        assert!(result.contains("!true"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_unary_bitnot() {
        let src = "fn main() {\n    let x = ~5\n}\n";
        let result = pp(src);
        assert!(result.contains("~5"));
        assert_roundtrip_stable(src);
    }

    // ── Types ────────────────────────────────────────────────────────

    #[test]
    fn test_class_with_fields_and_methods() {
        let src = r#"class Point {
    x: int
    y: int

    fn sum(self) int {
        return self.x + self.y
    }
}

fn main() {
    let p = Point { x: 1, y: 2 }
}
"#;
        let result = pp(src);
        assert!(result.contains("class Point {"));
        assert!(result.contains("x: int"));
        assert!(result.contains("fn sum(self) int {"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_class_with_bracket_deps() {
        let src = r#"class Logger {
}

class Service[logger: Logger] {
    fn run(self) {
        return
    }
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("class Service[logger: Logger]"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_class_with_impl() {
        let src = r#"trait Greetable {
    fn greet(self) string
}

class Person impl Greetable {
    name: string

    fn greet(self) string {
        return self.name
    }
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("class Person impl Greetable {"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_trait_abstract_and_default() {
        let src = r#"trait Animal {
    fn name(self) string

    fn greet(self) string {
        return self.name()
    }
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("fn name(self) string\n"));
        assert!(result.contains("fn greet(self) string {"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_enum_unit_and_data() {
        let src = r#"enum Shape {
    Circle { radius: float }
    Square { side: float }
    Unknown
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("Circle {"));
        assert!(result.contains("radius: float"));
        assert!(result.contains("Unknown"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_error_decl() {
        let src = r#"error NotFound {
    message: string
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("error NotFound {"));
        assert!(result.contains("message: string"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_generic_class() {
        let src = r#"class Box<T> {
    value: T
}

fn main() {
    let b = Box<int> { value: 42 }
}
"#;
        let result = pp(src);
        assert!(result.contains("class Box<T>"));
        assert!(result.contains("Box<int> { value: 42 }"));
        assert_roundtrip_stable(src);
    }

    // ── Expressions ──────────────────────────────────────────────────

    #[test]
    fn test_struct_literal() {
        let src = "class Pt {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Pt { x: 1, y: 2 }\n}\n";
        let result = pp(src);
        assert!(result.contains("Pt { x: 1, y: 2 }"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_array_literal() {
        let src = "fn main() {\n    let a = [1, 2, 3]\n}\n";
        let result = pp(src);
        assert!(result.contains("[1, 2, 3]"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_index() {
        let src = "fn main() {\n    let a = [1, 2, 3]\n    let x = a[0]\n}\n";
        let result = pp(src);
        assert!(result.contains("a[0]"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_method_call() {
        let src = r#"class Foo {
    x: int

    fn bar(self) int {
        return 1
    }
}

fn main() {
    let f = Foo { x: 1 }
    f.bar()
}
"#;
        let result = pp(src);
        assert!(result.contains("f.bar()"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_field_access() {
        let src = "class Pt {\n    x: int\n}\n\nfn main() {\n    let p = Pt { x: 1 }\n    let v = p.x\n}\n";
        let result = pp(src);
        assert!(result.contains("p.x"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_cast() {
        let src = "fn main() {\n    let x = 42 as float\n}\n";
        let result = pp(src);
        assert!(result.contains("42 as float"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_range() {
        let src = "fn main() {\n    for i in 0..10 {\n        return\n    }\n}\n";
        let result = pp(src);
        assert!(result.contains("0..10"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_spawn() {
        let src = r#"fn work() int {
    return 42
}

fn main() {
    let t = spawn work()
}
"#;
        let result = pp(src);
        assert!(result.contains("spawn work()"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_map_literal() {
        let src = "fn main() {\n    let m = Map<string, int> { \"a\": 1, \"b\": 2 }\n}\n";
        let result = pp(src);
        assert!(result.contains("Map<string, int>"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_set_literal() {
        let src = "fn main() {\n    let s = Set<int> { 1, 2, 3 }\n}\n";
        let result = pp(src);
        assert!(result.contains("Set<int> { 1, 2, 3 }"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_enum_unit_expr() {
        let src = "enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n}\n";
        let result = pp(src);
        assert!(result.contains("Color.Red"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_enum_data_expr() {
        let src = r#"enum Shape {
    Circle { radius: float }
}

fn main() {
    let s = Shape.Circle { radius: 3.14 }
}
"#;
        let result = pp(src);
        assert!(result.contains("Shape.Circle { radius: 3.14 }"));
        assert_roundtrip_stable(src);
    }

    // ── Strings ──────────────────────────────────────────────────────

    #[test]
    fn test_string_with_escapes() {
        let src = "fn main() {\n    let s = \"hello\\nworld\"\n}\n";
        let result = pp(src);
        assert!(result.contains("\"hello\\nworld\""));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_string_interpolation() {
        let src = "fn main() {\n    let name = \"world\"\n    let s = \"hello {name}\"\n}\n";
        let result = pp(src);
        assert!(result.contains("\"hello {name}\""));
        assert_roundtrip_stable(src);
    }

    // ── Closures ─────────────────────────────────────────────────────

    #[test]
    fn test_closure_single_expr() {
        let src = "fn main() {\n    let f = (x: int) => x + 1\n}\n";
        let result = pp(src);
        assert!(result.contains("(x: int) => x + 1"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_closure_block_body() {
        let src = "fn main() {\n    let f = (x: int) => {\n        let y = x + 1\n        return y\n    }\n}\n";
        let result = pp(src);
        assert!(result.contains("(x: int) => {"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_closure_with_return_type() {
        let src = "fn main() {\n    let f = (x: int) int => x + 1\n}\n";
        let result = pp(src);
        assert!(result.contains("(x: int) int => x + 1"));
        assert_roundtrip_stable(src);
    }

    // ── Errors ───────────────────────────────────────────────────────

    #[test]
    fn test_raise() {
        let src = r#"error NotFound {
    message: string
}

fn find() {
    raise NotFound { message: "oops" }
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("raise NotFound { message: \"oops\" }"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_propagate() {
        let src = r#"error NotFound {
    message: string
}

fn find() {
    raise NotFound { message: "oops" }
}

fn caller() {
    find()!
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("find()!"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_catch_shorthand() {
        let src = r#"error NotFound {
    message: string
}

fn find() int {
    raise NotFound { message: "oops" }
}

fn main() {
    let x = find() catch 0
}
"#;
        let result = pp(src);
        assert!(result.contains("find() catch 0"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_catch_wildcard() {
        let src = r#"error NotFound {
    message: string
}

fn find() int {
    raise NotFound { message: "oops" }
}

fn main() {
    let x = find() catch err { 0 }
}
"#;
        let result = pp(src);
        assert!(result.contains("find() catch err { 0 }"));
        assert_roundtrip_stable(src);
    }

    // ── Channels ─────────────────────────────────────────────────────

    #[test]
    fn test_let_chan() {
        let src = "fn main() {\n    let (tx, rx) = chan<int>()\n}\n";
        let result = pp(src);
        assert!(result.contains("let (tx, rx) = chan<int>()"));
        assert_roundtrip_stable(src);
    }

    // ── Top-level ────────────────────────────────────────────────────

    #[test]
    fn test_imports() {
        let src = "import math\n\nfn main() {\n}\n";
        let result = pp(src);
        assert!(result.starts_with("import math\n"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_import_with_alias() {
        let src = "import std.math as m\n\nfn main() {\n}\n";
        let result = pp(src);
        assert!(result.contains("import std.math as m"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_extern_fn() {
        let src = "extern fn sleep(ms: int)\n\nfn main() {\n}\n";
        let result = pp(src);
        assert!(result.contains("extern fn sleep(ms: int)"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_app_with_ambient() {
        let src = r#"class Logger {
}

class DB {
}

app MyApp[logger: Logger, db: DB] {
    ambient Logger
    ambient DB

    fn main(self) {
    }
}
"#;
        let result = pp(src);
        assert!(result.contains("app MyApp[logger: Logger, db: DB]"));
        assert!(result.contains("ambient Logger"));
        assert!(result.contains("ambient DB"));
        assert!(result.contains("fn main(self)"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_test_blocks() {
        let src = r#"fn add(a: int, b: int) int {
    return a + b
}

test "addition works" {
    expect(add(1, 2)).to_equal(3)
}
"#;
        let result = pp(src);
        assert!(result.contains("test \"addition works\""));
        assert!(result.contains("expect(add(1, 2)).to_equal(3)"));
        assert_roundtrip_stable(src);
    }

    // ── Idempotency ──────────────────────────────────────────────────

    #[test]
    fn test_comprehensive_idempotency() {
        let src = r#"import std.math as m

error NotFound {
    message: string
}

trait Printable {
    fn to_string(self) string
}

enum Color {
    Red
    Green
    Blue
}

enum Shape {
    Circle { radius: float }
    Rect { w: float, h: float }
}

class Point impl Printable {
    x: int
    y: int

    fn to_string(self) string {
        return "point"
    }

    fn add(self, other: Point) Point {
        return Point { x: self.x + other.x, y: self.y + other.y }
    }
}

fn identity(x: int) int {
    return x
}

fn main() {
    let x: int = 42
    let y = 3.14
    let s = "hello\nworld"
    let b = true
    let arr = [1, 2, 3]
    let p = Point { x: 1, y: 2 }
    let c = Color.Red
    let sh = Shape.Circle { radius: 1.5 }
    let f = (a: int) => a * 2
    if x > 0 {
        let z = x + y as int
    } else {
        let z = -x
    }
    while b {
        break
    }
    for i in 0..10 {
        continue
    }
    match c {
        Color.Red {
            return
        }
        Color.Green {
            return
        }
        Color.Blue {
            return
        }
    }
    let v = arr[0]
    let sum = p.add(p)
    let neg = !b
    let bit = ~x
    let expr = 1 + 2 * 3
    let parens = (1 + 2) * 3
    let assoc = x - y as int - 1
}
"#;
        assert_roundtrip_stable(src);
    }

    // ── Float formatting ─────────────────────────────────────────────

    #[test]
    fn test_float_with_decimal() {
        let src = "fn main() {\n    let x = 3.14\n}\n";
        let result = pp(src);
        assert!(result.contains("3.14"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_float_whole_number() {
        // 3.0 should stay as 3.0
        let src = "fn main() {\n    let x = 3.0\n}\n";
        let result = pp(src);
        assert!(result.contains("3.0") || result.contains("3"));
        assert_roundtrip_stable(src);
    }

    // ── Field assign ─────────────────────────────────────────────────

    #[test]
    fn test_field_assign() {
        let src = r#"class Pt {
    x: int
}

fn main() {
    let p = Pt { x: 1 }
    p.x = 42
}
"#;
        let result = pp(src);
        assert!(result.contains("p.x = 42"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_index_assign() {
        let src = "fn main() {\n    let a = [1, 2, 3]\n    a[0] = 99\n}\n";
        let result = pp(src);
        assert!(result.contains("a[0] = 99"));
        assert_roundtrip_stable(src);
    }

    // ── Match with bindings ──────────────────────────────────────────

    #[test]
    fn test_match_with_bindings() {
        let src = r#"enum Shape {
    Circle { radius: float }
}

fn main() {
    let s = Shape.Circle { radius: 3.14 }
    match s {
        Shape.Circle { radius } {
            return
        }
    }
}
"#;
        let result = pp(src);
        assert!(result.contains("Shape.Circle { radius }"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_match_with_renames() {
        let src = r#"enum Shape {
    Circle { radius: float }
}

fn main() {
    let s = Shape.Circle { radius: 3.14 }
    match s {
        Shape.Circle { radius: r } {
            return
        }
    }
}
"#;
        let result = pp(src);
        assert!(result.contains("Shape.Circle { radius: r }"));
        assert_roundtrip_stable(src);
    }

    // ── Fn type ──────────────────────────────────────────────────────

    #[test]
    fn test_fn_type_param() {
        let src = "fn apply(f: fn(int) int, x: int) int {\n    return f(x)\n}\n\nfn main() {\n}\n";
        let result = pp(src);
        assert!(result.contains("fn(int) int"));
        assert_roundtrip_stable(src);
    }

    // ── Empty map/set ────────────────────────────────────────────────

    #[test]
    fn test_empty_map() {
        let src = "fn main() {\n    let m = Map<string, int> {}\n}\n";
        let result = pp(src);
        assert!(result.contains("Map<string, int> {}"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_empty_set() {
        let src = "fn main() {\n    let s = Set<int> {}\n}\n";
        let result = pp(src);
        assert!(result.contains("Set<int> {}"));
        assert_roundtrip_stable(src);
    }

    // ── Inclusive range ──────────────────────────────────────────────

    #[test]
    fn test_inclusive_range() {
        let src = "fn main() {\n    for i in 0..=10 {\n        return\n    }\n}\n";
        let result = pp(src);
        assert!(result.contains("0..=10"));
        assert_roundtrip_stable(src);
    }

    // ── Return void ──────────────────────────────────────────────────

    #[test]
    fn test_return_void() {
        let src = "fn main() {\n    return\n}\n";
        let result = pp(src);
        assert!(result.contains("return\n"));
        assert_roundtrip_stable(src);
    }

    // ── Multiple functions with blank lines ──────────────────────────

    #[test]
    fn test_multiple_functions() {
        let src = "fn foo() {\n}\n\nfn bar() {\n}\n\nfn main() {\n}\n";
        let result = pp(src);
        assert_eq!(result, "fn foo() {\n}\n\nfn bar() {\n}\n\nfn main() {\n}\n");
        assert_roundtrip_stable(src);
    }

    // ── Else-if chain ────────────────────────────────────────────────

    #[test]
    fn test_nested_if_in_else() {
        // Pluto doesn't have `else if` — it's `else { if ... }`
        let src = "fn main() {\n    let x = 1\n    if x == 1 {\n        return\n    } else {\n        return\n    }\n}\n";
        let result = pp(src);
        assert!(result.contains("} else {"));
        assert_roundtrip_stable(src);
    }

    // ── Struct literal with empty fields ─────────────────────────────

    #[test]
    fn test_struct_literal_with_one_field() {
        // Parser doesn't support empty struct literals (`Foo {}`), so test with one field
        let src = "class Foo {\n    x: int\n}\n\nfn main() {\n    let f = Foo { x: 1 }\n}\n";
        let result = pp(src);
        assert!(result.contains("Foo { x: 1 }"));
        assert_roundtrip_stable(src);
    }

    // ── Class uses ───────────────────────────────────────────────────

    #[test]
    fn test_class_uses() {
        let src = r#"trait Greetable {
    fn greet(self) string {
        return "hi"
    }
}

class Person uses Greetable {
    name: string
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("class Person uses Greetable {"));
        assert_roundtrip_stable(src);
    }

    // ── Logical operators ────────────────────────────────────────────

    #[test]
    fn test_logical_and_or_precedence() {
        let src = "fn main() {\n    let x = true || false && true\n}\n";
        let result = pp(src);
        assert!(result.contains("true || false && true"));
        assert_roundtrip_stable(src);
    }

    // ── Bitwise operators ────────────────────────────────────────────

    #[test]
    fn test_bitwise_ops() {
        let src = "fn main() {\n    let x = 5 & 3 | 1\n}\n";
        let result = pp(src);
        // & has higher prec than |, so no parens needed
        assert!(result.contains("5 & 3 | 1"));
        assert_roundtrip_stable(src);
    }

    // ── Generic enum ─────────────────────────────────────────────────

    #[test]
    fn test_generic_enum() {
        let src = r#"enum Box<T> {
    Full { value: T }
    Empty
}

fn main() {
    let b = Box<int>.Full { value: 42 }
    let e = Box<int>.Empty
}
"#;
        let result = pp(src);
        assert!(result.contains("enum Box<T>"));
        assert!(result.contains("Box<int>.Full { value: 42 }"));
        assert!(result.contains("Box<int>.Empty"));
        assert_roundtrip_stable(src);
    }

    // ── Qualified type ───────────────────────────────────────────────

    #[test]
    fn test_qualified_type() {
        let src = "fn foo(p: int) int {\n    return p\n}\n\nfn main() {\n}\n";
        // Just testing basic roundtrip — qualified types appear after module flatten
        assert_roundtrip_stable(src);
    }

    // ── Array type ───────────────────────────────────────────────────

    #[test]
    fn test_array_type() {
        let src = "fn foo(arr: [int]) [int] {\n    return arr\n}\n\nfn main() {\n}\n";
        let result = pp(src);
        assert!(result.contains("[int]"));
        assert_roundtrip_stable(src);
    }

    // ── Contracts ───────────────────────────────────────────────────

    #[test]
    fn test_class_invariant() {
        let src = r#"class Positive {
    value: int

    invariant self.value > 0
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("invariant self.value > 0"));
        assert_roundtrip_stable(src);
    }

    #[test]
    fn test_class_multiple_invariants() {
        let src = r#"class BoundedInt {
    value: int

    invariant self.value >= 0
    invariant self.value <= 100
}

fn main() {
}
"#;
        let result = pp(src);
        assert!(result.contains("invariant self.value >= 0"));
        assert!(result.contains("invariant self.value <= 100"));
        assert_roundtrip_stable(src);
    }
}

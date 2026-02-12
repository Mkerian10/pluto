pub mod ast;

use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::lexer::token::Token;
use crate::span::{Span, Spanned};
use ast::*;

pub struct Parser<'a> {
    tokens: &'a [Spanned<Token>],
    source: &'a str,
    pos: usize,
    restrict_struct_lit: bool,
    enum_names: HashSet<String>,
    /// Optional file path for generating unique test IDs when multiple files are compiled together
    file_path: Option<String>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Spanned<Token>], source: &'a str) -> Self {
        // Seed with prelude enum names so all parse paths (including interpolation
        // sub-parsers) know about Option, Result, etc.
        let enum_names = crate::prelude::prelude_enum_names().clone();
        Self { tokens, source, pos: 0, restrict_struct_lit: false, enum_names, file_path: None }
    }

    /// Constructor without prelude seeding — used only to parse the prelude source itself.
    pub fn new_without_prelude(tokens: &'a [Spanned<Token>], source: &'a str) -> Self {
        Self { tokens, source, pos: 0, restrict_struct_lit: false, enum_names: HashSet::new(), file_path: None }
    }

    /// Constructor with extra enum names added to the prelude set.
    /// Used by the SDK editor to parse snippets that reference enums from the current program.
    pub fn new_with_enum_context(
        tokens: &'a [Spanned<Token>],
        source: &'a str,
        extra_enum_names: HashSet<String>,
    ) -> Self {
        let mut enum_names = crate::prelude::prelude_enum_names().clone();
        enum_names.extend(extra_enum_names);
        Self { tokens, source, pos: 0, restrict_struct_lit: false, enum_names, file_path: None }
    }

    /// Constructor with file path for generating unique test IDs
    pub fn new_with_path(tokens: &'a [Spanned<Token>], source: &'a str, file_path: String) -> Self {
        let enum_names = crate::prelude::prelude_enum_names().clone();
        Self { tokens, source, pos: 0, restrict_struct_lit: false, enum_names, file_path: Some(file_path) }
    }

    /// Generate a unique test ID prefix from file path to avoid collisions when multiple files are compiled together
    fn test_id_prefix(&self) -> String {
        match &self.file_path {
            Some(path) => {
                // Generate a short hash from the file path for uniqueness
                let mut hasher = DefaultHasher::new();
                path.hash(&mut hasher);
                let hash = hasher.finish();
                // Use first 8 hex digits of hash as prefix
                format!("{:08x}_", hash as u32)
            }
            None => String::new(),
        }
    }

    fn peek(&self) -> Option<&Spanned<Token>> {
        let mut i = self.pos;
        // Skip newlines when peeking
        while i < self.tokens.len() {
            if matches!(self.tokens[i].node, Token::Newline) {
                i += 1;
            } else {
                return Some(&self.tokens[i]);
            }
        }
        None
    }

    /// Peek at the nth token ahead (0-indexed, skipping newlines).
    fn peek_nth(&self, n: usize) -> Option<&Spanned<Token>> {
        let mut i = self.pos;
        let mut count = 0;
        while i < self.tokens.len() {
            if matches!(self.tokens[i].node, Token::Newline) {
                i += 1;
                continue;
            }
            if count == n {
                return Some(&self.tokens[i]);
            }
            count += 1;
            i += 1;
        }
        None
    }

    fn peek_raw(&self) -> Option<&Spanned<Token>> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Spanned<Token>> {
        if self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn skip_newlines(&mut self) {
        while self.pos < self.tokens.len() && matches!(self.tokens[self.pos].node, Token::Newline) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<&Spanned<Token>, CompileError> {
        self.skip_newlines();
        match self.tokens.get(self.pos) {
            Some(tok) if std::mem::discriminant(&tok.node) == std::mem::discriminant(expected) => {
                self.pos += 1;
                Ok(&self.tokens[self.pos - 1])
            }
            Some(tok) => Err(CompileError::syntax(
                format!("expected {expected}, found {}", tok.node),
                tok.span,
            )),
            None => Err(CompileError::syntax(
                format!("expected {expected}, found end of file"),
                self.eof_span(),
            )),
        }
    }

    fn expect_ident(&mut self) -> Result<Spanned<String>, CompileError> {
        self.skip_newlines();
        match self.tokens.get(self.pos) {
            Some(tok) if matches!(tok.node, Token::Ident) => {
                let name = self.source[tok.span.start..tok.span.end].to_string();
                self.pos += 1;
                Ok(Spanned::new(name, tok.span))
            }
            Some(tok) => Err(CompileError::syntax(
                format!("expected identifier, found {}", tok.node),
                tok.span,
            )),
            None => Err(CompileError::syntax(
                "expected identifier, found end of file",
                self.eof_span(),
            )),
        }
    }

    fn eof_span(&self) -> Span {
        if let Some(last) = self.tokens.last() {
            Span::new(last.span.end, last.span.end)
        } else {
            Span::dummy()
        }
    }

    fn at_statement_boundary(&self) -> bool {
        // Check if we're at a newline or at end of input
        match self.peek_raw() {
            None => true,
            Some(tok) => matches!(tok.node, Token::Newline),
        }
    }

    fn consume_statement_end(&mut self) {
        // Consume a newline if present, or we're at } or EOF
        if let Some(tok) = self.peek_raw() && matches!(tok.node, Token::Newline) {
            self.advance();
        }
    }

    /// Parse a comma-separated list of items until `close` delimiter is reached.
    /// Assumes the opening delimiter has already been consumed and `skip_newlines()` called.
    /// If `mandatory_comma` is true, commas between items are required (function args);
    /// otherwise commas are optional (struct fields, set elements).
    /// Handles trailing commas in both modes.
    fn parse_comma_list<T>(
        &mut self,
        close: &Token,
        mandatory_comma: bool,
        mut parse_item: impl FnMut(&mut Self) -> Result<T, CompileError>,
    ) -> Result<Vec<T>, CompileError> {
        let mut items = Vec::new();
        while self.peek().is_some()
            && std::mem::discriminant(&self.peek().expect("token should exist after is_some check").node) != std::mem::discriminant(close)
        {
            if !items.is_empty() {
                if mandatory_comma {
                    self.expect(&Token::Comma)?;
                } else if self.peek().is_some()
                    && matches!(self.peek().expect("token should exist after is_some check").node, Token::Comma)
                {
                    self.advance();
                }
                self.skip_newlines();
                if self.peek().is_some()
                    && std::mem::discriminant(&self.peek().expect("token should exist after is_some check").node)
                        == std::mem::discriminant(close)
                {
                    break; // trailing comma
                }
            }
            items.push(parse_item(self)?);
            self.skip_newlines();
        }
        Ok(items)
    }

    /// Parse `name: expr, ...` field list inside `{ }`. Assumes `{` already consumed.
    /// Returns fields and the closing `}` span end.
    #[allow(clippy::type_complexity)]
    fn parse_field_list(&mut self) -> Result<(Vec<(Spanned<String>, Spanned<Expr>)>, usize), CompileError> {
        self.skip_newlines();
        let fields = self.parse_comma_list(&Token::RBrace, false, |p| {
            let fname = p.expect_ident()?;
            p.expect(&Token::Colon)?;
            let fval = p.parse_expr(0)?;
            Ok((fname, fval))
        })?;
        let close = self.expect(&Token::RBrace)?;
        Ok((fields, close.span.end))
    }

    fn pre_scan_enum_names(&mut self) {
        let saved = self.pos;
        let mut i = 0;
        while i < self.tokens.len() {
            let is_enum = matches!(self.tokens[i].node, Token::Enum);
            let is_pub_enum = i + 1 < self.tokens.len()
                && matches!(self.tokens[i].node, Token::Pub)
                && matches!(self.tokens[i + 1].node, Token::Enum);
            if is_enum || is_pub_enum {
                let name_idx = if is_pub_enum { i + 2 } else { i + 1 };
                if name_idx < self.tokens.len() && matches!(self.tokens[name_idx].node, Token::Ident) {
                    let name = self.source[self.tokens[name_idx].span.start..self.tokens[name_idx].span.end].to_string();
                    self.enum_names.insert(name);
                }
            }
            i += 1;
        }
        self.pos = saved;
    }

    pub fn parse_program(&mut self) -> Result<Program, CompileError> {
        self.pre_scan_enum_names();
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut extern_fns = Vec::new();
        let mut classes = Vec::new();
        let mut traits = Vec::new();
        let mut enums = Vec::new();
        let mut app = None;
        let mut stages = Vec::new();
        let mut system = None;
        let mut errors = Vec::new();
        let mut test_info: Vec<TestInfo> = Vec::new();
        let mut tests: Option<Spanned<TestsDecl>> = None;
        self.skip_newlines();

        // Parse imports first
        while self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Import) {
            imports.push(self.parse_import()?);
            self.skip_newlines();
        }

        while let Some(tok) = self.peek() {
            // Handle `pub` modifier
            let is_pub = if matches!(tok.node, Token::Pub) {
                self.advance(); // consume 'pub'
                self.skip_newlines();
                true
            } else {
                false
            };

            // Parse optional lifecycle modifier: scoped | transient
            let lifecycle = match self.peek().map(|t| &t.node) {
                Some(Token::Scoped) => {
                    self.advance();
                    self.skip_newlines();
                    Lifecycle::Scoped
                }
                Some(Token::Transient) => {
                    self.advance();
                    self.skip_newlines();
                    Lifecycle::Transient
                }
                Some(_) => Lifecycle::Singleton,
                None => {
                    return Err(CompileError::syntax(
                        "expected declaration after 'pub'", self.eof_span(),
                    ));
                }
            };

            let tok = self.peek().ok_or_else(|| {
                CompileError::syntax(
                    if lifecycle != Lifecycle::Singleton {
                        "expected 'class' after lifecycle modifier"
                    } else {
                        "expected declaration"
                    },
                    self.eof_span(),
                )
            })?;

            match &tok.node {
                Token::App => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    if is_pub {
                        return Err(CompileError::syntax(
                            "app declarations cannot be pub",
                            tok.span,
                        ));
                    }
                    let app_decl = self.parse_app_decl()?;
                    if app.is_some() {
                        return Err(CompileError::syntax(
                            "duplicate app declaration",
                            app_decl.span,
                        ));
                    }
                    app = Some(app_decl);
                }
                Token::Class => {
                    let mut class = self.parse_class()?;
                    class.node.is_pub = is_pub;
                    class.node.lifecycle = lifecycle;
                    classes.push(class);
                }
                Token::Fn => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    let mut func = self.parse_function()?;
                    func.node.is_pub = is_pub;
                    functions.push(func);
                }
                Token::Trait => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    let mut tr = self.parse_trait()?;
                    tr.node.is_pub = is_pub;
                    traits.push(tr);
                }
                Token::Enum => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    let mut e = self.parse_enum_decl()?;
                    e.node.is_pub = is_pub;
                    enums.push(e);
                }
                Token::Error => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    let mut err_decl = self.parse_error_decl()?;
                    err_decl.node.is_pub = is_pub;
                    errors.push(err_decl);
                }
                Token::Extern => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    // Only extern fn is supported
                    let next = self.peek_nth(1);
                    if matches!(next, Some(t) if matches!(t.node, Token::Fn)) {
                        extern_fns.push(self.parse_extern_fn(is_pub)?);
                    } else {
                        return Err(CompileError::syntax(
                            "expected 'fn' after 'extern'",
                            tok.span,
                        ));
                    }
                }
                Token::Tests => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    if is_pub {
                        return Err(CompileError::syntax(
                            "tests declarations cannot be pub",
                            tok.span,
                        ));
                    }
                    let (tests_decl, block_tests, block_functions) = self.parse_tests_decl(&test_info, &functions)?;
                    if !test_info.is_empty() {
                        return Err(CompileError::syntax(
                            "cannot mix bare 'test' blocks with 'tests' declarations",
                            tests_decl.span,
                        ));
                    }
                    test_info.extend(block_tests);
                    functions.extend(block_functions);
                    tests = Some(tests_decl);
                }
                Token::Test => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    if is_pub {
                        return Err(CompileError::syntax(
                            "tests cannot be pub",
                            tok.span,
                        ));
                    }
                    if tests.is_some() {
                        return Err(CompileError::syntax(
                            "cannot mix bare 'test' blocks with 'tests' declarations",
                            tok.span,
                        ));
                    }
                    let (info, func) = self.parse_single_test(&test_info, &functions)?;
                    test_info.push(info);
                    functions.push(func);
                }
                Token::System => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    if is_pub {
                        return Err(CompileError::syntax(
                            "system declarations cannot be pub",
                            tok.span,
                        ));
                    }
                    let system_decl = self.parse_system_decl()?;
                    if system.is_some() {
                        return Err(CompileError::syntax(
                            "duplicate system declaration",
                            system_decl.span,
                        ));
                    }
                    system = Some(system_decl);
                }
                Token::Stage => {
                    if lifecycle != Lifecycle::Singleton {
                        return Err(CompileError::syntax(
                            "lifecycle modifiers (scoped, transient) can only be used on classes",
                            tok.span,
                        ));
                    }
                    if is_pub {
                        return Err(CompileError::syntax(
                            "stage declarations cannot be pub",
                            tok.span,
                        ));
                    }
                    let stage_decl = self.parse_stage_decl()?;
                    stages.push(stage_decl);
                }
                _ => {
                    return Err(CompileError::syntax(
                        format!("expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found {}", tok.node),
                        tok.span,
                    ));
                }
            }
            self.skip_newlines();
        }

        // Reject system + app in same file
        if system.is_some() && app.is_some() {
            let app_span = app.as_ref().unwrap().span;
            return Err(CompileError::syntax(
                "a file cannot contain both 'system' and 'app' declarations",
                app_span,
            ));
        }

        // Reject tests + app in same file
        if tests.is_some() && app.is_some() {
            let tests_span = tests.as_ref().unwrap().span;
            return Err(CompileError::syntax(
                "a file cannot contain both 'tests' and 'app' declarations",
                tests_span,
            ));
        }

        // Reject tests + system in same file
        if tests.is_some() && system.is_some() {
            let tests_span = tests.as_ref().unwrap().span;
            return Err(CompileError::syntax(
                "a file cannot contain both 'tests' and 'system' declarations",
                tests_span,
            ));
        }

        // Reject stage + app in same file
        if !stages.is_empty() && app.is_some() {
            let app_span = app.as_ref().unwrap().span;
            return Err(CompileError::syntax(
                "a file cannot contain both 'stage' and 'app' declarations",
                app_span,
            ));
        }

        // Reject stage + system in same file
        if !stages.is_empty() && system.is_some() {
            let system_span = system.as_ref().unwrap().span;
            return Err(CompileError::syntax(
                "a file cannot contain both 'stage' and 'system' declarations",
                system_span,
            ));
        }

        Ok(Program { imports, functions, extern_fns,  classes, traits, enums, app, stages, system, errors, test_info, tests, fallible_extern_fns: Vec::new() })
    }

    /// Parse a bare `test "name" { body }` block into a TestInfo + synthetic Function.
    fn parse_single_test(&mut self, existing_tests: &[TestInfo], _existing_fns: &[Spanned<Function>]) -> Result<(TestInfo, Spanned<Function>), CompileError> {
        let test_tok = self.expect(&Token::Test)?;
        let start = test_tok.span.start;
        let test_span = test_tok.span;

        // Expect string literal for test name
        let name_tok = self.advance().ok_or_else(|| {
            CompileError::syntax("expected test name (string literal) after 'test'", test_span)
        })?;
        let display_name = match &name_tok.node {
            Token::StringLit(s) => s.clone(),
            _ => {
                return Err(CompileError::syntax(
                    "expected test name (string literal) after 'test'",
                    name_tok.span,
                ));
            }
        };

        // Check for duplicate test names
        if existing_tests.iter().any(|t| t.display_name == display_name) {
            return Err(CompileError::syntax(
                format!("duplicate test name '{}'", display_name),
                name_tok.span,
            ));
        }

        // Parse test body
        self.skip_newlines();
        let body = self.parse_block()?;
        let end = body.span.end;

        let test_index = existing_tests.len();
        let fn_name = format!("__test_{}{}", self.test_id_prefix(), test_index);
        let info = TestInfo {
            display_name,
            fn_name: fn_name.clone(),
        };
        let func = Spanned::new(Function {
            id: Uuid::new_v4(),
            name: Spanned::new(fn_name, Span::new(start, end)),
            type_params: Vec::new(),
            type_param_bounds: HashMap::new(),
            params: Vec::new(),
            return_type: None,
            contracts: Vec::new(),
            body,
            is_pub: false,
            is_override: false,
            is_generator: false,
        }, Span::new(start, end));

        Ok((info, func))
    }

    /// Parse `tests[scheduler: Strategy] { test "name" { ... } ... }`
    fn parse_tests_decl(&mut self, existing_tests: &[TestInfo], existing_fns: &[Spanned<Function>]) -> Result<(Spanned<TestsDecl>, Vec<TestInfo>, Vec<Spanned<Function>>), CompileError> {
        let tests_tok = self.expect(&Token::Tests)?;
        let start = tests_tok.span.start;

        // Parse bracket dep: [scheduler: Ident]
        self.expect(&Token::LBracket)?;
        let key_tok = self.expect_ident()?;
        if key_tok.node != "scheduler" {
            return Err(CompileError::syntax(
                format!("expected 'scheduler' in tests bracket, found '{}'", key_tok.node),
                key_tok.span,
            ));
        }
        self.expect(&Token::Colon)?;

        // Expect strategy name: Sequential, RoundRobin, Random, Exhaustive
        let strategy_tok = self.expect_ident()?;
        let strategy = match strategy_tok.node.as_str() {
            "Sequential" => "Sequential".to_string(),
            "RoundRobin" => "RoundRobin".to_string(),
            "Random" => "Random".to_string(),
            "Exhaustive" => "Exhaustive".to_string(),
            other => {
                return Err(CompileError::syntax(
                    format!("unknown scheduler strategy '{}' (expected Sequential, RoundRobin, Random, or Exhaustive)", other),
                    strategy_tok.span,
                ));
            }
        };
        self.expect(&Token::RBracket)?;

        // Parse body: { test "name" { ... } ... }
        self.skip_newlines();
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut block_tests = Vec::new();
        let mut block_functions = Vec::new();

        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            // Only test blocks are allowed inside tests { ... }
            let inner_tok = self.peek().unwrap();
            if !matches!(inner_tok.node, Token::Test) {
                return Err(CompileError::syntax(
                    "only 'test' blocks are allowed inside a 'tests' declaration",
                    inner_tok.span,
                ));
            }
            let combined_tests: Vec<TestInfo> = existing_tests.iter().chain(block_tests.iter()).cloned().collect();
            let combined_fns: Vec<Spanned<Function>> = existing_fns.iter().chain(block_functions.iter()).cloned().collect();
            let (info, func) = self.parse_single_test(&combined_tests, &combined_fns)?;
            block_tests.push(info);
            block_functions.push(func);
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        let decl = Spanned::new(TestsDecl {
            id: Uuid::new_v4(),
            strategy,
        }, Span::new(start, end));

        Ok((decl, block_tests, block_functions))
    }

    fn parse_import(&mut self) -> Result<Spanned<ImportDecl>, CompileError> {
        let import_tok = self.expect(&Token::Import)?;
        let start = import_tok.span.start;
        let first = self.expect_ident()?;
        let mut path = vec![first];

        // Parse dotted path segments: import std.io.fs
        // Use peek_raw() so a newline stops the path (prevents `import a\n.b` from parsing as `import a.b`)
        while self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Dot) {
            self.advance(); // consume '.'
            let segment = self.expect_ident()?;
            path.push(segment);
        }

        let mut end = path.last().unwrap().span.end;

        // Parse optional alias: `as name`
        let alias = if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::As) {
            self.advance(); // consume 'as'
            let alias_name = self.expect_ident()?;
            end = alias_name.span.end;
            Some(alias_name)
        } else {
            None
        };

        self.consume_statement_end();
        Ok(Spanned::new(ImportDecl { path, alias }, Span::new(start, end)))
    }

    fn parse_extern_fn(&mut self, is_pub: bool) -> Result<Spanned<ExternFnDecl>, CompileError> {
        let extern_tok = self.expect(&Token::Extern)?;
        let start = extern_tok.span.start;
        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_comma_list(&Token::RParen, true, |p| {
            let pname = p.expect_ident()?;
            p.expect(&Token::Colon)?;
            let pty = p.parse_type()?;
            Ok(Param { id: Uuid::new_v4(), name: pname, ty: pty, is_mut: false })
        })?;
        let close_paren = self.expect(&Token::RParen)?;
        let mut end = close_paren.span.end;

        // Optional return type — if next raw token is not newline/EOF, parse return type
        let return_type = if !self.at_statement_boundary()
            && self.peek().is_some()
            && !matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace)
        {
            let ty = self.parse_type()?;
            end = ty.span.end;
            Some(ty)
        } else {
            None
        };

        self.consume_statement_end();
        Ok(Spanned::new(ExternFnDecl { name, params, return_type, is_pub }, Span::new(start, end)))
    }

    fn parse_bracket_deps(&mut self) -> Result<Vec<Field>, CompileError> {
        if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBracket) {
            self.advance(); // consume '['
            let deps = self.parse_comma_list(&Token::RBracket, true, |p| {
                let name = p.expect_ident()?;
                p.expect(&Token::Colon)?;
                let ty = p.parse_type()?;
                Ok(Field { id: Uuid::new_v4(), name, ty, is_injected: true, is_ambient: false })
            })?;
            self.expect(&Token::RBracket)?;
            Ok(deps)
        } else {
            Ok(Vec::new())
        }
    }

    fn parse_app_decl(&mut self) -> Result<Spanned<AppDecl>, CompileError> {
        let app_tok = self.expect(&Token::App)?;
        let start = app_tok.span.start;
        let name = self.expect_ident()?;

        let inject_fields = self.parse_bracket_deps()?;

        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        // Parse ambient declarations, lifecycle overrides, and methods
        let mut ambient_types = Vec::new();
        let mut lifecycle_overrides = Vec::new();
        let mut methods = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            if matches!(self.peek().expect("token should exist after is_some check").node, Token::Ambient) {
                self.advance(); // consume 'ambient'
                ambient_types.push(self.expect_ident()?);
                self.consume_statement_end();
            } else if matches!(self.peek().expect("token should exist after is_some check").node, Token::Scoped) {
                self.advance(); // consume 'scoped'
                let class_name = self.expect_ident()?;
                lifecycle_overrides.push((class_name, Lifecycle::Scoped));
                self.consume_statement_end();
            } else if matches!(self.peek().expect("token should exist after is_some check").node, Token::Transient) {
                self.advance(); // consume 'transient'
                let class_name = self.expect_ident()?;
                lifecycle_overrides.push((class_name, Lifecycle::Transient));
                self.consume_statement_end();
            } else {
                methods.push(self.parse_method()?);
            }
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(AppDecl { id: Uuid::new_v4(), name, inject_fields, ambient_types, lifecycle_overrides, methods }, Span::new(start, end)))
    }

    fn parse_stage_decl(&mut self) -> Result<Spanned<StageDecl>, CompileError> {
        let stage_tok = self.expect(&Token::Stage)?;
        let start = stage_tok.span.start;
        let name = self.expect_ident()?;

        // Parse optional parent: `stage Worker : Daemon`
        let parent = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Colon) {
            self.advance(); // consume ':'
            Some(self.expect_ident()?)
        } else {
            None
        };

        let inject_fields = self.parse_bracket_deps()?;

        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        // Parse ambient declarations, lifecycle overrides, required methods, and methods
        let mut ambient_types = Vec::new();
        let mut lifecycle_overrides = Vec::new();
        let mut required_methods = Vec::new();
        let mut methods = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            if matches!(self.peek().expect("token should exist after is_some check").node, Token::Ambient) {
                self.advance(); // consume 'ambient'
                ambient_types.push(self.expect_ident()?);
                self.consume_statement_end();
            } else if matches!(self.peek().expect("token should exist after is_some check").node, Token::Scoped) {
                self.advance(); // consume 'scoped'
                let class_name = self.expect_ident()?;
                lifecycle_overrides.push((class_name, Lifecycle::Scoped));
                self.consume_statement_end();
            } else if matches!(self.peek().expect("token should exist after is_some check").node, Token::Transient) {
                self.advance(); // consume 'transient'
                let class_name = self.expect_ident()?;
                lifecycle_overrides.push((class_name, Lifecycle::Transient));
                self.consume_statement_end();
            } else {
                // Parse optional 'pub' before methods/requires
                let is_pub = if matches!(self.peek().expect("token should exist after is_some check").node, Token::Pub) {
                    self.advance();
                    true
                } else {
                    false
                };

                if matches!(self.peek().expect("token should exist after is_some check").node, Token::Requires) {
                    // `requires fn name(self, ...) ReturnType`
                    let mut req = self.parse_required_method()?;
                    req.node.is_pub = is_pub;
                    required_methods.push(req);
                } else if matches!(self.peek().expect("token should exist after is_some check").node, Token::Override) {
                    // `override fn name(self, ...) { ... }`
                    self.advance(); // consume 'override'
                    let mut method = self.parse_method()?;
                    method.node.is_pub = is_pub;
                    method.node.is_override = true;
                    methods.push(method);
                } else {
                    let mut method = self.parse_method()?;
                    method.node.is_pub = is_pub;
                    methods.push(method);
                }
            }
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(StageDecl { id: Uuid::new_v4(), name, parent, inject_fields, ambient_types, lifecycle_overrides, required_methods, methods }, Span::new(start, end)))
    }

    fn parse_required_method(&mut self) -> Result<Spanned<RequiredMethod>, CompileError> {
        let req_tok = self.expect(&Token::Requires)?;
        let start = req_tok.span.start;
        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        let mut first = true;
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RParen) {
            if !params.is_empty() || !first {
                self.expect(&Token::Comma)?;
            }
            first = false;

            // Handle `self` / `mut self` as first param
            if params.is_empty() && self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Mut) {
                let mut_tok = self.advance().expect("token should exist after peek");
                let mut_span = mut_tok.span;
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::SelfVal) {
                    let self_tok = self.advance().expect("token should exist after peek");
                    params.push(Param {
                        id: Uuid::new_v4(),
                        name: Spanned::new("self".to_string(), self_tok.span),
                        ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                        is_mut: true,
                    });
                } else {
                    return Err(CompileError::syntax("expected 'self' after 'mut'", mut_span));
                }
            } else if params.is_empty() && self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::SelfVal) {
                let self_tok = self.advance().expect("token should exist after peek");
                params.push(Param {
                    id: Uuid::new_v4(),
                    name: Spanned::new("self".to_string(), self_tok.span),
                    ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                    is_mut: false,
                });
            } else {
                let pname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let pty = self.parse_type()?;
                params.push(Param { id: Uuid::new_v4(), name: pname, ty: pty, is_mut: false });
            }
        }
        let rparen = self.expect(&Token::RParen)?;
        let rparen_end = rparen.span.end;

        // Optional return type — anything that's NOT a newline, `{`, `}`, `requires`, `ensures`, `fn`, `override`, `pub`
        let return_type = if self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node,
            Token::Newline | Token::LBrace | Token::RBrace | Token::Requires | Token::Ensures | Token::Fn | Token::Override | Token::Pub
        ) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let end = return_type.as_ref().map(|rt| rt.span.end).unwrap_or(rparen_end);
        self.consume_statement_end();

        Ok(Spanned::new(RequiredMethod {
            id: Uuid::new_v4(),
            name,
            params,
            return_type,
            is_pub: false,
        }, Span::new(start, end)))
    }

    fn parse_system_decl(&mut self) -> Result<Spanned<SystemDecl>, CompileError> {
        let system_tok = self.expect(&Token::System)?;
        let start = system_tok.span.start;
        let name = self.expect_ident()?;

        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut members = Vec::new();
        let mut seen_names = HashSet::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            let member_name = self.expect_ident()?;
            if !seen_names.insert(member_name.node.clone()) {
                return Err(CompileError::syntax(
                    format!("duplicate system member name: '{}'", member_name.node),
                    member_name.span,
                ));
            }
            self.expect(&Token::Colon)?;
            let module_name = self.expect_ident()?;
            members.push(SystemMember {
                id: Uuid::new_v4(),
                name: member_name,
                module_name,
            });
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(SystemDecl { id: Uuid::new_v4(), name, members }, Span::new(start, end)))
    }

    fn parse_enum_decl(&mut self) -> Result<Spanned<EnumDecl>, CompileError> {
        let enum_tok = self.expect(&Token::Enum)?;
        let start = enum_tok.span.start;
        let name = self.expect_ident()?;
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut variants = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            let vname = self.expect_ident()?;
            let fields = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace) {
                self.expect(&Token::LBrace)?;
                self.skip_newlines();
                let fields = self.parse_comma_list(&Token::RBrace, false, |p| {
                    let fname = p.expect_ident()?;
                    p.expect(&Token::Colon)?;
                    let fty = p.parse_type()?;
                    Ok(Field { id: Uuid::new_v4(), name: fname, ty: fty, is_injected: false, is_ambient: false })
                })?;
                self.expect(&Token::RBrace)?;
                fields
            } else {
                Vec::new()
            };
            variants.push(EnumVariant { id: Uuid::new_v4(), name: vname, fields });
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(EnumDecl { id: Uuid::new_v4(), name, type_params, type_param_bounds, variants, is_pub: false }, Span::new(start, end)))
    }

    fn parse_error_decl(&mut self) -> Result<Spanned<ErrorDecl>, CompileError> {
        let err_tok = self.expect(&Token::Error)?;
        let start = err_tok.span.start;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            if !fields.is_empty() {
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Comma) {
                    self.advance();
                }
                self.skip_newlines();
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
                    break;
                }
            }
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let fty = self.parse_type()?;
            fields.push(Field { id: Uuid::new_v4(), name: fname, ty: fty, is_injected: false, is_ambient: false });
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(ErrorDecl { id: Uuid::new_v4(), name, fields, is_pub: false }, Span::new(start, end)))
    }

    fn parse_trait(&mut self) -> Result<Spanned<TraitDecl>, CompileError> {
        let trait_tok = self.expect(&Token::Trait)?;
        let start = trait_tok.span.start;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            methods.push(self.parse_trait_method()?);
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(TraitDecl { id: Uuid::new_v4(), name, methods, is_pub: false }, Span::new(start, end)))
    }

    fn parse_trait_method(&mut self) -> Result<TraitMethod, CompileError> {
        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        let mut first = true;
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RParen) {
            if !params.is_empty() || !first {
                self.expect(&Token::Comma)?;
            }
            first = false;

            if params.is_empty() && self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Mut) {
                let mut_tok = self.advance().expect("token should exist after peek");
                let mut_span = mut_tok.span;
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::SelfVal) {
                    let self_tok = self.advance().expect("token should exist after peek");
                    params.push(Param {
                        id: Uuid::new_v4(),
                        name: Spanned::new("self".to_string(), self_tok.span),
                        ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                        is_mut: true,
                    });
                } else {
                    return Err(CompileError::syntax(
                        "expected 'self' after 'mut'",
                        mut_span,
                    ));
                }
            } else if params.is_empty() && self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::SelfVal) {
                let self_tok = self.advance().expect("token should exist after peek");
                params.push(Param {
                    id: Uuid::new_v4(),
                    name: Spanned::new("self".to_string(), self_tok.span),
                    ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                    is_mut: false,
                });
            } else {
                let pname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let pty = self.parse_type()?;
                params.push(Param { id: Uuid::new_v4(), name: pname, ty: pty, is_mut: false });
            }
        }
        self.expect(&Token::RParen)?;

        // Check for return type - use peek_raw() to detect newline boundary
        let return_type = if let Some(next_raw) = self.peek_raw() && matches!(next_raw.node, Token::Newline | Token::RBrace | Token::Requires | Token::Ensures) {
            // Newline, closing brace, or contract - no return type
            None
        } else if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace) {
            // Opening brace (method body) - no return type
            None
        } else if self.peek().is_some() {
            // Parse return type
            Some(self.parse_type()?)
        } else {
            None
        };

        // Parse optional requires/ensures contracts
        let contracts = self.parse_contracts()?;

        // If next token is '{', parse a body (default implementation)
        let body = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace) {
            Some(self.parse_block()?)
        } else {
            if contracts.is_empty() {
                self.consume_statement_end();
            }
            None
        };

        Ok(TraitMethod { id: Uuid::new_v4(), name, params, return_type, contracts, body })
    }

    fn parse_class(&mut self) -> Result<Spanned<ClassDecl>, CompileError> {
        let class_tok = self.expect(&Token::Class)?;
        let start = class_tok.span.start;
        let name = self.expect_ident()?;
        let (type_params, type_param_bounds) = self.parse_type_params()?;

        // Parse optional `uses Type, Type2`
        let uses = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Uses) {
            self.advance(); // consume 'uses'
            let mut types = Vec::new();
            types.push(self.expect_ident()?);
            while self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Comma) {
                self.advance(); // consume ','
                types.push(self.expect_ident()?);
            }
            types
        } else {
            Vec::new()
        };

        // Parse optional bracket deps: class Foo[dep: Type]
        let inject_fields = self.parse_bracket_deps()?;

        // Check for `impl Trait1, Trait2`
        let impl_traits = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Impl) {
            self.advance(); // consume 'impl'
            let mut traits = Vec::new();
            traits.push(self.expect_ident()?);
            while self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Comma) {
                self.advance(); // consume ','
                traits.push(self.expect_ident()?);
            }
            traits
        } else {
            Vec::new()
        };

        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        // Injected fields come first, then regular body fields
        let mut fields: Vec<Field> = inject_fields;
        let mut methods = Vec::new();
        let mut invariants = Vec::new();

        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            if matches!(self.peek().expect("token should exist after is_some check").node, Token::Fn) {
                methods.push(self.parse_method()?);
            } else if matches!(self.peek().expect("token should exist after is_some check").node, Token::Invariant) {
                let inv_tok = self.advance().expect("token should exist after peek");
                let inv_start = inv_tok.span.start;
                let expr = self.parse_expr(0)?;
                let inv_end = expr.span.end;
                invariants.push(Spanned::new(
                    ContractClause { kind: ContractKind::Invariant, expr },
                    Span::new(inv_start, inv_end),
                ));
                self.consume_statement_end();
            } else {
                let fname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let fty = self.parse_type()?;
                fields.push(Field { id: Uuid::new_v4(), name: fname, ty: fty, is_injected: false, is_ambient: false });
                self.consume_statement_end();
            }
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(ClassDecl { id: Uuid::new_v4(), name, type_params, type_param_bounds, fields, methods, invariants, impl_traits, uses, is_pub: false, lifecycle: Lifecycle::Singleton }, Span::new(start, end)))
    }

    fn parse_method(&mut self) -> Result<Spanned<Function>, CompileError> {
        let fn_tok = self.expect(&Token::Fn)?;
        let start = fn_tok.span.start;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        let mut first = true;
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RParen) {
            if !params.is_empty() || !first {
                self.expect(&Token::Comma)?;
            }
            first = false;

            // Check for `mut self` or `self` as first param
            if params.is_empty() && self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Mut) {
                let mut_tok = self.advance().expect("token should exist after peek");
                let mut_span = mut_tok.span;
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::SelfVal) {
                    let self_tok = self.advance().expect("token should exist after peek");
                    params.push(Param {
                        id: Uuid::new_v4(),
                        name: Spanned::new("self".to_string(), self_tok.span),
                        ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                        is_mut: true,
                    });
                } else {
                    return Err(CompileError::syntax(
                        "expected 'self' after 'mut'",
                        mut_span,
                    ));
                }
            } else if params.is_empty() && self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::SelfVal) {
                let self_tok = self.advance().expect("token should exist after peek");
                params.push(Param {
                    id: Uuid::new_v4(),
                    name: Spanned::new("self".to_string(), self_tok.span),
                    ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                    is_mut: false,
                });
            } else {
                let pname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let pty = self.parse_type()?;
                params.push(Param { id: Uuid::new_v4(), name: pname, ty: pty, is_mut: false });
            }
        }
        self.expect(&Token::RParen)?;

        let return_type = if self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace | Token::Requires | Token::Ensures) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // Reject generator methods (Phase 1: generators are top-level functions only)
        if return_type.as_ref().is_some_and(|rt| matches!(rt.node, TypeExpr::Stream(_))) {
            return Err(CompileError::syntax(
                "generator methods are not supported; generators must be top-level functions".to_string(),
                return_type.as_ref().unwrap().span,
            ));
        }

        let contracts = self.parse_contracts()?;

        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Function { id: Uuid::new_v4(), name, type_params: vec![], type_param_bounds: HashMap::new(), params, return_type, contracts, body, is_pub: false, is_override: false, is_generator: false },
            Span::new(start, end),
        ))
    }

    /// Parse optional type parameters: `<T>`, `<A, B>`, `<T: Trait1 + Trait2>`, or empty.
    /// Returns (type_param_names, bounds_map).
    fn parse_type_params(&mut self) -> Result<(Vec<Spanned<String>>, HashMap<String, Vec<Spanned<String>>>), CompileError> {
        if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Lt) {
            self.advance(); // consume '<'
            let mut params = Vec::new();
            let mut bounds = HashMap::new();
            let result = self.parse_comma_list(&Token::Gt, true, |p| {
                let name = p.expect_ident()?;
                // Check for `: Trait1 + Trait2` bounds
                if p.peek().is_some() && matches!(p.peek().expect("token should exist after is_some check").node, Token::Colon) {
                    p.advance(); // consume ':'
                    let mut trait_bounds = Vec::new();
                    trait_bounds.push(p.expect_ident()?);
                    while p.peek().is_some() && matches!(p.peek().expect("token should exist after is_some check").node, Token::Plus) {
                        p.advance(); // consume '+'
                        trait_bounds.push(p.expect_ident()?);
                    }
                    Ok((name, trait_bounds))
                } else {
                    Ok((name, Vec::new()))
                }
            })?;
            for (name, trait_bounds) in result {
                if !trait_bounds.is_empty() {
                    bounds.insert(name.node.clone(), trait_bounds);
                }
                params.push(name);
            }
            self.expect(&Token::Gt)?;
            Ok((params, bounds))
        } else {
            Ok((Vec::new(), HashMap::new()))
        }
    }

    /// Parse a type argument list: `<int, string>`, etc. Assumes we're positioned at `<`.
    fn parse_type_arg_list(&mut self) -> Result<Vec<Spanned<TypeExpr>>, CompileError> {
        self.expect(&Token::Lt)?;
        let args = self.parse_comma_list(&Token::Gt, true, |p| p.parse_type())?;
        self.expect(&Token::Gt)?;
        Ok(args)
    }

    /// Parse optional requires/ensures clauses before a function body.
    fn parse_contracts(&mut self) -> Result<Vec<Spanned<ContractClause>>, CompileError> {
        let mut contracts = Vec::new();
        while let Some(tok) = self.peek() {
            match &tok.node {
                Token::Requires => {
                    self.skip_newlines();
                    let req_tok = self.advance().expect("token should exist after peek");
                    let req_start = req_tok.span.start;
                    let expr = self.parse_expr(0)?;
                    let req_end = expr.span.end;
                    contracts.push(Spanned::new(
                        ContractClause { kind: ContractKind::Requires, expr },
                        Span::new(req_start, req_end),
                    ));
                    self.consume_statement_end();
                }
                Token::Ensures => {
                    self.skip_newlines();
                    let ens_tok = self.advance().expect("token should exist after peek");
                    let ens_start = ens_tok.span.start;
                    let expr = self.parse_expr(0)?;
                    let ens_end = expr.span.end;
                    contracts.push(Spanned::new(
                        ContractClause { kind: ContractKind::Ensures, expr },
                        Span::new(ens_start, ens_end),
                    ));
                    self.consume_statement_end();
                }
                _ => break,
            }
        }
        Ok(contracts)
    }

    fn parse_function(&mut self) -> Result<Spanned<Function>, CompileError> {
        let fn_tok = self.expect(&Token::Fn)?;
        let start = fn_tok.span.start;
        let name = self.expect_ident()?;
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_comma_list(&Token::RParen, true, |p| {
            let pname = p.expect_ident()?;
            p.expect(&Token::Colon)?;
            let pty = p.parse_type()?;
            Ok(Param { id: Uuid::new_v4(), name: pname, ty: pty, is_mut: false })
        })?;
        self.expect(&Token::RParen)?;

        // Return type: if next non-newline token is not '{' or a contract keyword, it's a return type
        let return_type = if self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace | Token::Requires | Token::Ensures) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let contracts = self.parse_contracts()?;

        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Function {
                id: Uuid::new_v4(), name, type_params, type_param_bounds, params,
                is_generator: return_type.as_ref().is_some_and(|rt| matches!(rt.node, TypeExpr::Stream(_))),
                return_type, contracts, body, is_pub: false, is_override: false,
            },
            Span::new(start, end),
        ))
    }

    fn parse_type(&mut self) -> Result<Spanned<TypeExpr>, CompileError> {
        self.skip_newlines();
        let mut result = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Stream) {
            // Stream type: stream T
            let stream_tok = self.advance().expect("token should exist after peek");
            let start = stream_tok.span.start;
            let inner = self.parse_type()?;
            let end = inner.span.end;
            Ok(Spanned::new(TypeExpr::Stream(Box::new(inner)), Span::new(start, end)))
        } else if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBracket) {
            let open = self.advance().expect("token should exist after peek");
            let start = open.span.start;
            let inner = self.parse_type()?;
            let close = self.expect(&Token::RBracket)?;
            let end = close.span.end;
            Ok(Spanned::new(TypeExpr::Array(Box::new(inner)), Span::new(start, end)))
        } else if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Fn) {
            // Function type: fn(int, float) string
            let fn_tok = self.advance().expect("token should exist after peek");
            let start = fn_tok.span.start;
            self.expect(&Token::LParen)?;
            let params = self.parse_comma_list(&Token::RParen, true, |p| {
                Ok(Box::new(p.parse_type()?))
            })?;
            let close_paren = self.expect(&Token::RParen)?;
            let mut end = close_paren.span.end;
            // Optional return type — if next token looks like a type, parse it; otherwise void
            let return_type = if !self.at_statement_boundary()
                && self.peek().is_some()
                && !matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace | Token::Comma | Token::RParen | Token::FatArrow | Token::RBracket | Token::Eq)
            {
                let ty = self.parse_type()?;
                end = ty.span.end;
                Box::new(ty)
            } else {
                Box::new(Spanned::new(TypeExpr::Named("void".to_string()), Span::new(end, end)))
            };
            Ok(Spanned::new(TypeExpr::Fn { params, return_type }, Span::new(start, end)))
        } else {
            let ident = self.expect_ident()?;
            // Check for qualified type: module.Type
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Dot) {
                self.advance(); // consume '.'
                let type_name = self.expect_ident()?;
                let qualified_name = format!("{}.{}", ident.node, type_name.node);
                let end_span = type_name.span.end;
                // Check for generic type args on qualified type: module.Type<int, string>
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Lt) {
                    let type_args = self.parse_type_arg_list()?;
                    let end = type_args.last().map_or(end_span, |_| self.tokens[self.pos - 1].span.end);
                    Ok(Spanned::new(TypeExpr::Generic { name: qualified_name, type_args }, Span::new(ident.span.start, end)))
                } else {
                    let span = Span::new(ident.span.start, end_span);
                    Ok(Spanned::new(TypeExpr::Qualified { module: ident.node, name: type_name.node }, span))
                }
            } else if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Lt) {
                // Generic type: Type<int, string>
                let start = ident.span.start;
                let type_args = self.parse_type_arg_list()?;
                let end = self.tokens[self.pos - 1].span.end; // end of '>'
                Ok(Spanned::new(TypeExpr::Generic { name: ident.node, type_args }, Span::new(start, end)))
            } else {
                Ok(Spanned::new(TypeExpr::Named(ident.node), ident.span))
            }
        }?;
        // Trailing ? makes this a nullable type: T?
        if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Question) {
            let q = self.advance().expect("token should exist after peek");
            let start = result.span.start;
            result = Spanned::new(TypeExpr::Nullable(Box::new(result)), Span::new(start, q.span.end));
        }
        Ok(result)
    }

    fn parse_block(&mut self) -> Result<Spanned<Block>, CompileError> {
        let open = self.expect(&Token::LBrace)?;
        let start = open.span.start;
        let mut stmts = Vec::new();

        self.skip_newlines();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(Block { stmts }, Span::new(start, end)))
    }

    fn parse_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let tok = self.peek().ok_or_else(|| {
            CompileError::syntax("unexpected end of file", self.eof_span())
        })?;

        match &tok.node {
            Token::Let => self.parse_let_stmt(),
            Token::Return => self.parse_return_stmt(),
            Token::Yield => self.parse_yield_stmt(),
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::For => self.parse_for_stmt(),
            Token::Match => self.parse_match_stmt(),
            Token::Select => self.parse_select_stmt(),
            Token::Scope => self.parse_scope_stmt(),
            Token::Raise => self.parse_raise_stmt(),
            Token::Break => {
                let span = self.advance().expect("token should exist after peek").span;
                self.consume_statement_end();
                Ok(Spanned::new(Stmt::Break, span))
            }
            Token::Continue => {
                let span = self.advance().expect("token should exist after peek").span;
                self.consume_statement_end();
                Ok(Spanned::new(Stmt::Continue, span))
            }
            _ => {
                // Parse a full expression, then check for `=`, compound assignment,
                // or `++`/`--` to determine statement kind.
                let start = tok.span.start;
                let expr = self.parse_expr(0)?;

                // Check for compound assignment operator
                let compound_op = self.peek().and_then(|t| match &t.node {
                    Token::PlusEq => Some(BinOp::Add),
                    Token::MinusEq => Some(BinOp::Sub),
                    Token::StarEq => Some(BinOp::Mul),
                    Token::SlashEq => Some(BinOp::Div),
                    Token::PercentEq => Some(BinOp::Mod),
                    _ => None,
                });

                if let Some(op) = compound_op {
                    self.advance(); // consume compound assignment token
                    let rhs = self.parse_expr(0)?;
                    let end = rhs.span.end;
                    self.consume_statement_end();
                    // Desugar: x += y  =>  x = x + y
                    return self.desugar_compound_assign(expr, op, rhs, start, end);
                }

                // Check for ++ / --
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::PlusPlus | Token::MinusMinus) {
                    let inc_tok = self.advance().expect("token should exist after peek");
                    let op = if matches!(inc_tok.node, Token::PlusPlus) { BinOp::Add } else { BinOp::Sub };
                    let end = inc_tok.span.end;
                    self.consume_statement_end();
                    // Desugar: x++  =>  x = x + 1
                    let one = Spanned::new(Expr::IntLit(1), Span::new(end, end));
                    return self.desugar_compound_assign(expr, op, one, start, end);
                }

                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Eq) {
                    self.advance(); // consume '='
                    let value = self.parse_expr(0)?;
                    let end = value.span.end;
                    self.consume_statement_end();

                    match expr.node {
                        Expr::Ident(name) => {
                            Ok(Spanned::new(
                                Stmt::Assign {
                                    target: Spanned::new(name, expr.span),
                                    value,
                                },
                                Span::new(start, end),
                            ))
                        }
                        Expr::FieldAccess { object, field } => {
                            Ok(Spanned::new(
                                Stmt::FieldAssign {
                                    object: *object,
                                    field,
                                    value,
                                },
                                Span::new(start, end),
                            ))
                        }
                        Expr::Index { object, index } => {
                            Ok(Spanned::new(
                                Stmt::IndexAssign {
                                    object: *object,
                                    index: *index,
                                    value,
                                },
                                Span::new(start, end),
                            ))
                        }
                        _ => Err(CompileError::syntax(
                            "invalid assignment target",
                            expr.span,
                        )),
                    }
                } else {
                    let end = expr.span.end;
                    self.consume_statement_end();
                    Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
                }
            }
        }
    }

    /// Desugar compound assignment: `x += y` => `x = x + y`, also handles `x++` => `x = x + 1`.
    /// Supports variable, field, and index targets.
    fn desugar_compound_assign(
        &self,
        target_expr: Spanned<Expr>,
        op: BinOp,
        rhs: Spanned<Expr>,
        start: usize,
        end: usize,
    ) -> Result<Spanned<Stmt>, CompileError> {
        let span = Span::new(start, end);
        // Build `target op rhs` expression using a clone of the target as the LHS
        let bin_expr = Spanned::new(
            Expr::BinOp {
                op,
                lhs: Box::new(target_expr.clone()),
                rhs: Box::new(rhs),
            },
            span,
        );
        match target_expr.node {
            Expr::Ident(name) => Ok(Spanned::new(
                Stmt::Assign {
                    target: Spanned::new(name, target_expr.span),
                    value: bin_expr,
                },
                span,
            )),
            Expr::FieldAccess { object, field } => Ok(Spanned::new(
                Stmt::FieldAssign {
                    object: *object,
                    field,
                    value: bin_expr,
                },
                span,
            )),
            Expr::Index { object, index } => Ok(Spanned::new(
                Stmt::IndexAssign {
                    object: *object,
                    index: *index,
                    value: bin_expr,
                },
                span,
            )),
            _ => Err(CompileError::syntax(
                "invalid compound assignment target",
                target_expr.span,
            )),
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let let_tok = self.expect(&Token::Let)?;
        let start = let_tok.span.start;

        // Check for `mut` keyword
        let is_mut = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Mut) {
            self.advance(); // consume `mut`
            true
        } else {
            false
        };

        // Check for destructuring: let (tx, rx) = chan<T>(...)
        if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LParen) {
            return self.parse_let_chan(start);
        }

        let name = self.expect_ident()?;

        let ty = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Colon) {
            self.advance(); // consume ':'
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&Token::Eq)?;
        let value = self.parse_expr(0)?;
        let end = value.span.end;
        self.consume_statement_end();

        Ok(Spanned::new(Stmt::Let { name, ty, value, is_mut }, Span::new(start, end)))
    }

    fn parse_let_chan(&mut self, start: usize) -> Result<Spanned<Stmt>, CompileError> {
        self.expect(&Token::LParen)?;
        let sender = self.expect_ident()?;
        self.expect(&Token::Comma)?;
        let receiver = self.expect_ident()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::Eq)?;

        // Expect `chan`
        let chan_ident = self.expect_ident()?;
        if chan_ident.node != "chan" {
            return Err(CompileError::syntax(
                "expected `chan<T>()` after `let (tx, rx) =`".to_string(),
                chan_ident.span,
            ));
        }

        // Parse <T>
        self.expect(&Token::Lt)?;
        let elem_type = self.parse_type()?;
        self.expect(&Token::Gt)?;

        // Parse ( [capacity] )
        self.expect(&Token::LParen)?;
        let capacity = if self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RParen) {
            Some(self.parse_expr(0)?)
        } else {
            None
        };
        let close = self.expect(&Token::RParen)?;
        let end = close.span.end;
        self.consume_statement_end();

        Ok(Spanned::new(
            Stmt::LetChan { sender, receiver, elem_type, capacity },
            Span::new(start, end),
        ))
    }

    fn parse_return_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let ret_span = self.expect(&Token::Return)?.span;
        let start = ret_span.start;

        // Check if there's a value to return (not newline/rbrace/eof)
        let value = if self.at_statement_boundary() || self.peek().is_some_and(|t| matches!(t.node, Token::RBrace)) {
            None
        } else {
            Some(self.parse_expr(0)?)
        };

        let end = value.as_ref().map_or(ret_span.end, |v| v.span.end);
        self.consume_statement_end();

        Ok(Spanned::new(Stmt::Return(value), Span::new(start, end)))
    }

    fn parse_yield_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let yield_span = self.expect(&Token::Yield)?.span;
        let start = yield_span.start;

        // yield always requires a value expression
        let value = self.parse_expr(0)?;
        let end = value.span.end;
        self.consume_statement_end();

        Ok(Spanned::new(Stmt::Yield { value }, Span::new(start, end)))
    }

    fn parse_if_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let if_tok = self.expect(&Token::If)?;
        let start = if_tok.span.start;
        let old_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let condition = self.parse_expr(0)?;
        self.restrict_struct_lit = old_restrict;
        let then_block = self.parse_block()?;

        let else_block = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Else) {
            self.advance(); // consume 'else'
            // Desugar `else if` into `else { if ... }`
            if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::If) {
                let nested_if = self.parse_if_stmt()?;
                let span = nested_if.span;
                Some(Spanned::new(
                    Block { stmts: vec![nested_if] },
                    span,
                ))
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };

        let end = else_block.as_ref().map_or(then_block.span.end, |b| b.span.end);

        Ok(Spanned::new(
            Stmt::If { condition, then_block, else_block },
            Span::new(start, end),
        ))
    }

    fn parse_while_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let while_tok = self.expect(&Token::While)?;
        let start = while_tok.span.start;
        let old_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let condition = self.parse_expr(0)?;
        self.restrict_struct_lit = old_restrict;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Stmt::While { condition, body },
            Span::new(start, end),
        ))
    }

    fn parse_for_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let for_tok = self.expect(&Token::For)?;
        let start = for_tok.span.start;
        let var = self.expect_ident()?;
        self.expect(&Token::In)?;
        let old_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let iterable = self.parse_expr(0)?;
        self.restrict_struct_lit = old_restrict;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Stmt::For { var, iterable, body },
            Span::new(start, end),
        ))
    }

    fn parse_match_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let match_tok = self.expect(&Token::Match)?;
        let start = match_tok.span.start;
        let old_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let scrutinee = self.parse_expr(0)?;
        self.restrict_struct_lit = old_restrict;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut arms = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            let first_name = self.expect_ident()?;
            self.expect(&Token::Dot)?;
            let second_name = self.expect_ident()?;

            // Check if this is module.Enum.Variant (qualified) or Enum.Variant (local)
            let (enum_name, variant_name) = if self.peek().is_some()
                && matches!(self.peek().expect("token should exist after is_some check").node, Token::Dot)
            {
                // module.Enum.Variant — consume the extra dot and variant
                self.advance(); // consume '.'
                let variant = self.expect_ident()?;
                let qualified = format!("{}.{}", first_name.node, second_name.node);
                let span = Span::new(first_name.span.start, second_name.span.end);
                (Spanned::new(qualified, span), variant)
            } else {
                // Enum.Variant (local)
                (first_name, second_name)
            };

            let (bindings, body) = if self.is_match_bindings_ahead() {
                // Parse bindings: { field_name, field_name: rename }
                self.expect(&Token::LBrace)?;
                self.skip_newlines();
                let mut bindings = Vec::new();
                while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
                    if !bindings.is_empty() {
                        if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Comma) {
                            self.advance();
                        }
                        self.skip_newlines();
                        if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
                            break;
                        }
                    }
                    let field_name = self.expect_ident()?;
                    let rename = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Colon) {
                        self.advance();
                        Some(self.expect_ident()?)
                    } else {
                        None
                    };
                    bindings.push((field_name, rename));
                    self.skip_newlines();
                }
                self.expect(&Token::RBrace)?;
                let body = self.parse_block()?;
                (bindings, body)
            } else {
                let body = self.parse_block()?;
                (Vec::new(), body)
            };

            arms.push(MatchArm { enum_name, variant_name, type_args: vec![], bindings, body, enum_id: None, variant_id: None });
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(Stmt::Match { expr: scrutinee, arms }, Span::new(start, end)))
    }

    fn is_match_bindings_ahead(&self) -> bool {
        // We need to distinguish between:
        //   Status.Active { print("active") }  -- unit arm, body block
        //   Status.Suspended { reason } { print(reason) }  -- bindings then body
        // Look for pattern: { (ident (: ident)? ,?)* } {
        if self.pos >= self.tokens.len() || !matches!(self.tokens[self.pos].node, Token::LBrace) {
            return false;
        }
        // Skip past newlines after the opening brace
        let mut i = self.pos + 1;
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        // Scan tokens: must only be Ident, Comma, Colon, Newline until we hit }
        loop {
            if i >= self.tokens.len() {
                return false;
            }
            match &self.tokens[i].node {
                Token::RBrace => {
                    // Found closing }. Now check if next non-newline is {
                    i += 1;
                    while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
                        i += 1;
                    }
                    return i < self.tokens.len() && matches!(self.tokens[i].node, Token::LBrace);
                }
                Token::Ident | Token::Comma | Token::Colon | Token::Newline => {
                    i += 1;
                }
                _ => return false,
            }
        }
    }

    fn parse_select_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let select_tok = self.expect(&Token::Select)?;
        let start = select_tok.span.start;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut arms = Vec::new();
        let mut default_block = None;

        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            // Check for `default { ... }`
            if matches!(self.peek().expect("token should exist after is_some check").node, Token::Default) {
                self.advance(); // consume 'default'
                if default_block.is_some() {
                    return Err(CompileError::syntax(
                        "duplicate default arm in select",
                        self.eof_span(),
                    ));
                }
                default_block = Some(self.parse_block()?);
                self.skip_newlines();
                continue;
            }

            // Parse a select arm: either recv or send
            // Recv: `binding = expr.recv() { body }`
            // Send: `expr.send(value) { body }`
            //
            // To distinguish: peek ahead for `ident = ...` pattern
            let is_recv = self.is_select_recv_ahead();

            if is_recv {
                // Recv arm: `binding = expr.recv() { body }`
                let binding = self.expect_ident()?;
                self.expect(&Token::Eq)?;
                let channel = self.parse_expr(0)?;
                // The expr should end with a .recv() method call — validate structure
                // We parse the entire `rx.recv()` as an expression, then unwrap
                match &channel.node {
                    Expr::MethodCall { object, method, args } if method.node == "recv" && args.is_empty() => {
                        let channel_expr = (**object).clone();
                        let body = self.parse_block()?;
                        arms.push(SelectArm {
                            op: SelectOp::Recv { binding, channel: channel_expr },
                            body,
                        });
                    }
                    _ => {
                        return Err(CompileError::syntax(
                            "select recv arm must be: binding = channel.recv() { ... }",
                            channel.span,
                        ));
                    }
                }
            } else {
                // Send arm: `expr.send(value) { body }`
                let expr = self.parse_expr(0)?;
                match &expr.node {
                    Expr::MethodCall { object, method, args } if method.node == "send" && args.len() == 1 => {
                        let channel_expr = (**object).clone();
                        let value_expr = args[0].clone();
                        let body = self.parse_block()?;
                        arms.push(SelectArm {
                            op: SelectOp::Send { channel: channel_expr, value: value_expr },
                            body,
                        });
                    }
                    _ => {
                        return Err(CompileError::syntax(
                            "select send arm must be: channel.send(value) { ... }",
                            expr.span,
                        ));
                    }
                }
            }

            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        if arms.is_empty() && default_block.is_none() {
            return Err(CompileError::syntax(
                "select must have at least one arm or a default",
                Span::new(start, end),
            ));
        }

        Ok(Spanned::new(Stmt::Select { arms, default: default_block }, Span::new(start, end)))
    }

    fn parse_scope_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let scope_tok = self.expect(&Token::Scope)?;
        let start = scope_tok.span.start;
        self.expect(&Token::LParen)?;

        // Parse comma-separated seed expressions.
        // No struct-lit restriction needed here since seeds are inside parentheses.
        let old_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = false;
        let mut seeds = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RParen) {
                break;
            }
            if !seeds.is_empty() {
                self.expect(&Token::Comma)?;
                self.skip_newlines();
                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RParen) {
                    break;
                }
            }
            seeds.push(self.parse_expr(0)?);
        }
        self.restrict_struct_lit = old_restrict;
        self.expect(&Token::RParen)?;

        // Parse pipe-delimited bindings: |name: Type, name2: Type2|
        self.expect(&Token::Pipe)?;
        let mut bindings = Vec::new();
        loop {
            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Pipe) {
                break;
            }
            if !bindings.is_empty() {
                self.expect(&Token::Comma)?;
            }
            let name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            bindings.push(ScopeBinding { name, ty });
        }
        self.expect(&Token::Pipe)?;

        if seeds.is_empty() {
            return Err(CompileError::syntax(
                "scope block requires at least one seed expression",
                Span::new(start, start + 5),
            ));
        }
        if bindings.is_empty() {
            return Err(CompileError::syntax(
                "scope block requires at least one binding",
                Span::new(start, start + 5),
            ));
        }

        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Stmt::Scope { seeds, bindings, body },
            Span::new(start, end),
        ))
    }

    /// Check if the next tokens form a recv pattern: `ident = ...`
    fn is_select_recv_ahead(&self) -> bool {
        // Look for: Ident followed by `=` (not `==`)
        let mut i = self.pos;
        // Skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Ident) {
            return false;
        }
        i += 1;
        // Skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() {
            return false;
        }
        matches!(self.tokens[i].node, Token::Eq)
    }

    fn parse_raise_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let raise_tok = self.expect(&Token::Raise)?;
        let start = raise_tok.span.start;
        let first = self.expect_ident()?;

        // Check for qualified name: raise module.ErrorName { ... }
        let error_name = if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Dot) {
            self.advance(); // consume '.'
            let second = self.expect_ident()?;
            let span = Span::new(first.span.start, second.span.end);
            Spanned::new(format!("{}.{}", first.node, second.node), span)
        } else {
            first
        };

        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
            if !fields.is_empty() {
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::Comma) {
                    self.advance();
                }
                self.skip_newlines();
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::RBrace) {
                    break;
                }
            }
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let fval = self.parse_expr(0)?;
            fields.push((fname, fval));
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;
        self.consume_statement_end();

        Ok(Spanned::new(
            Stmt::Raise { error_name, fields, error_id: None },
            Span::new(start, end),
        ))
    }

    fn parse_catch_handler(&mut self) -> Result<(CatchHandler, usize), CompileError> {
        // Lookahead: if ident followed by {, it's wildcard form
        if self.is_catch_wildcard_ahead() {
            let var = self.expect_ident()?;
            let body = self.parse_block()?;
            let end = body.span.end;
            Ok((CatchHandler::Wildcard { var, body }, end))
        } else {
            let fallback = self.parse_expr(0)?;
            let end = fallback.span.end;
            Ok((CatchHandler::Shorthand(Box::new(fallback)), end))
        }
    }

    fn is_catch_wildcard_ahead(&self) -> bool {
        let mut i = self.pos;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        // Must be ident
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Ident) {
            return false;
        }
        i += 1;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        // Must be {
        i < self.tokens.len() && matches!(self.tokens[i].node, Token::LBrace)
    }

    // Pratt parser for expressions
    fn parse_expr(&mut self, min_bp: u8) -> Result<Spanned<Expr>, CompileError> {
        let mut lhs = self.parse_prefix()?;

        while let Some(tok) = self.peek().cloned() {

            // Dot notation (postfix) — highest precedence
            if matches!(tok.node, Token::Dot) {
                self.skip_newlines();
                self.advance(); // consume '.'
                let field_name = self.expect_ident()?;

                // Check if it's a method call
                if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LParen) {
                    self.advance(); // consume '('
                    self.skip_newlines();
                    let args = self.parse_comma_list(&Token::RParen, true, |p| p.parse_expr(0))?;
                    let close = self.expect(&Token::RParen)?;
                    let span = Span::new(lhs.span.start, close.span.end);
                    lhs = Spanned::new(
                        Expr::MethodCall {
                            object: Box::new(lhs),
                            method: field_name,
                            args,
                        },
                        span,
                    );
                } else if matches!(&lhs.node, Expr::Ident(n) if self.enum_names.contains(n)) {
                    // Enum construction: EnumName.Variant or EnumName.Variant { field: value }
                    let enum_name_str = match &lhs.node {
                        Expr::Ident(n) => n.clone(),
                        _ => unreachable!(),
                    };
                    let enum_name_span = lhs.span;
                    if !self.restrict_struct_lit
                        && self.peek().is_some()
                        && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace)
                        && self.is_struct_lit_ahead()
                    {
                        // EnumName.Variant { field: value }
                        self.advance(); // consume '{'
                        let (fields, close_end) = self.parse_field_list()?;
                        let span = Span::new(lhs.span.start, close_end);
                        lhs = Spanned::new(
                            Expr::EnumData {
                                enum_name: Spanned::new(enum_name_str, enum_name_span),
                                variant: field_name,
                                type_args: vec![],
                                fields,
                                enum_id: None,
                                variant_id: None,
                            },
                            span,
                        );
                    } else {
                        // EnumName.Variant (unit)
                        let span = Span::new(lhs.span.start, field_name.span.end);
                        lhs = Spanned::new(
                            Expr::EnumUnit {
                                enum_name: Spanned::new(enum_name_str, enum_name_span),
                                variant: field_name,
                                type_args: vec![],
                                enum_id: None,
                                variant_id: None,
                            },
                            span,
                        );
                    }
                } else if !self.restrict_struct_lit
                    && matches!(&lhs.node, Expr::Ident(_))
                    && self.peek().is_some()
                    && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace)
                    && self.is_struct_lit_ahead()
                {
                    // Qualified struct literal: module.Type { field: value }
                    let module_name = match &lhs.node {
                        Expr::Ident(n) => n.clone(),
                        _ => unreachable!(),
                    };
                    let qualified_name = format!("{}.{}", module_name, field_name.node);
                    let name_span = Span::new(lhs.span.start, field_name.span.end);

                    self.advance(); // consume '{'
                    let (fields, close_end) = self.parse_field_list()?;
                    let span = Span::new(lhs.span.start, close_end);
                    lhs = Spanned::new(
                        Expr::StructLit {
                            name: Spanned::new(qualified_name, name_span),
                            type_args: vec![],
                            fields,
                            target_id: None,
                        },
                        span,
                    );
                } else {
                    // All other cases: regular field access (obj.field, obj.inner.field, etc.)
                    // Special case: a.b.c { fields } could be module.Enum.Variant { fields }
                    // We defer the decision to the rewrite pass by creating a special node
                    let span = Span::new(lhs.span.start, field_name.span.end);
                    lhs = Spanned::new(
                        Expr::FieldAccess {
                            object: Box::new(lhs),
                            field: field_name.clone(),
                        },
                        span,
                    );

                    // Check if this FieldAccess is followed by { ... } for EnumData
                    // Pattern: a.b.c { fields } where a is module, b is Enum, c is Variant
                    if !self.restrict_struct_lit
                        && matches!(&lhs.node, Expr::FieldAccess { object, .. } if matches!(&object.node, Expr::FieldAccess { .. }))
                        && self.peek().is_some()
                        && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace)
                        && self.is_struct_lit_ahead()
                    {
                        // Create a temporary EnumData node - the rewrite pass will validate
                        // whether this is actually an enum or needs to be converted back to field access
                        let (module_name, enum_local) = match &lhs.node {
                            Expr::FieldAccess { object, field: _variant } => {
                                match &object.node {
                                    Expr::FieldAccess { object: inner_obj, field: inner_field } => {
                                        match &inner_obj.node {
                                            Expr::Ident(m) => (m.clone(), inner_field.node.clone()),
                                            _ => {
                                                // Not the pattern we're looking for - just continue
                                                continue;
                                            }
                                        }
                                    }
                                    _ => {
                                        // Not the pattern we're looking for - just continue
                                        continue;
                                    }
                                }
                            }
                            _ => unreachable!(),
                        };

                        let qualified_enum_name = format!("{}.{}", module_name, enum_local);
                        let enum_name_span = Span::new(lhs.span.start - field_name.node.len() - 1, field_name.span.end);

                        self.advance(); // consume '{'
                        let (fields, close_end) = self.parse_field_list()?;
                        let span = Span::new(lhs.span.start, close_end);
                        lhs = Spanned::new(
                            Expr::EnumData {
                                enum_name: Spanned::new(qualified_enum_name, enum_name_span),
                                variant: field_name,
                                type_args: vec![],
                                fields,
                                enum_id: None,
                                variant_id: None,
                            },
                            span,
                        );
                    }
                }
                continue;
            }

            // Index operator: arr[i] — allow across newlines like method calls
            // This enables: let x = arr\n    [0]
            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBracket) {
                self.skip_newlines(); // Skip newlines before consuming '['
                self.advance(); // consume '['
                let index = self.parse_expr(0)?;
                let close = self.expect(&Token::RBracket)?;
                let span = Span::new(lhs.span.start, close.span.end);
                lhs = Spanned::new(
                    Expr::Index {
                        object: Box::new(lhs),
                        index: Box::new(index),
                    },
                    span,
                );
                continue;
            }

            // Postfix ? — null propagation (unwrap nullable, early-return none)
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Question) {
                let q = self.advance().expect("token should exist after peek");
                let span = Span::new(lhs.span.start, q.span.end);
                lhs = Spanned::new(
                    Expr::NullPropagate { expr: Box::new(lhs) },
                    span,
                );
                continue;
            }

            // Postfix ! — error propagation (must be on same line via peek_raw)
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Bang) {
                let bang = self.advance().expect("token should exist after peek");
                let span = Span::new(lhs.span.start, bang.span.end);
                lhs = Spanned::new(
                    Expr::Propagate { expr: Box::new(lhs) },
                    span,
                );
                continue;
            }

            // Postfix catch — error handling (must be on same line via peek_raw)
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Catch) {
                self.advance(); // consume 'catch'
                let (handler, end) = self.parse_catch_handler()?;
                let span = Span::new(lhs.span.start, end);
                lhs = Spanned::new(
                    Expr::Catch { expr: Box::new(lhs), handler },
                    span,
                );
                continue;
            }

            // Postfix as — type cast (binds tighter than all infix operators, but
            // looser than prefix unary ops so that `-1 as byte` = `(-1) as byte`)
            if min_bp < 21 && self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::As) {
                self.advance(); // consume 'as'
                let target_type = self.parse_type()?;
                let span = Span::new(lhs.span.start, target_type.span.end);
                lhs = Spanned::new(
                    Expr::Cast {
                        expr: Box::new(lhs),
                        target_type,
                    },
                    span,
                );
                continue;
            }

            // Check for generic enum expression: EnumName<type_args>.Variant
            if matches!(&lhs.node, Expr::Ident(n) if self.enum_names.contains(n))
                && matches!(tok.node, Token::Lt)
                && self.is_generic_enum_expr_ahead()
            {
                let enum_name_str = match &lhs.node {
                    Expr::Ident(n) => n.clone(),
                    _ => unreachable!(),
                };
                let enum_name_span = lhs.span;
                let type_args = self.parse_type_arg_list()?;
                self.expect(&Token::Dot)?;
                let variant = self.expect_ident()?;

                if !self.restrict_struct_lit
                    && self.peek().is_some()
                    && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace)
                    && self.is_struct_lit_ahead()
                {
                    // EnumName<type_args>.Variant { field: value }
                    self.advance(); // consume '{'
                    let (fields, close_end) = self.parse_field_list()?;
                    let span = Span::new(lhs.span.start, close_end);
                    lhs = Spanned::new(
                        Expr::EnumData {
                            enum_name: Spanned::new(enum_name_str, enum_name_span),
                            variant,
                            type_args,
                            fields,
                            enum_id: None,
                            variant_id: None,
                        },
                        span,
                    );
                } else {
                    let span = Span::new(lhs.span.start, variant.span.end);
                    lhs = Spanned::new(
                        Expr::EnumUnit {
                            enum_name: Spanned::new(enum_name_str, enum_name_span),
                            variant,
                            type_args,
                            enum_id: None,
                            variant_id: None,
                        },
                        span,
                    );
                }
                continue;
            }

            // Range: `..` (exclusive) or `..=` (inclusive)
            if matches!(tok.node, Token::DotDot | Token::DotDotEq) {
                let inclusive = matches!(tok.node, Token::DotDotEq);
                self.advance(); // consume `..` or `..=`
                let rhs = self.parse_expr(0)?;
                let span = Span::new(lhs.span.start, rhs.span.end);
                lhs = Spanned::new(
                    Expr::Range {
                        start: Box::new(lhs),
                        end: Box::new(rhs),
                        inclusive,
                    },
                    span,
                );
                break; // ranges don't chain
            }

            // Right shift: detect adjacent `>` `>` as `>>` without adding a lexer token.
            if matches!(tok.node, Token::Gt)
                && self.tokens.get(self.pos + 1).is_some_and(|next| {
                    matches!(next.node, Token::Gt) && tok.span.end == next.span.start
                })
            {
                let op = BinOp::Shr;
                let (lbp, rbp) = infix_binding_power(op);
                if lbp < min_bp {
                    break;
                }

                self.advance(); // first '>'
                self.advance(); // second '>'

                let rhs = self.parse_expr(rbp)?;
                let span = Span::new(lhs.span.start, rhs.span.end);
                lhs = Spanned::new(
                    Expr::BinOp {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    span,
                );
                continue;
            }

            let op = match &tok.node {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                Token::EqEq => BinOp::Eq,
                Token::BangEq => BinOp::Neq,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::LtEq => BinOp::LtEq,
                Token::GtEq => BinOp::GtEq,
                Token::AmpAmp => BinOp::And,
                Token::PipePipe => BinOp::Or,
                Token::Amp => BinOp::BitAnd,
                Token::Pipe => BinOp::BitOr,
                Token::Caret => BinOp::BitXor,
                Token::Shl => BinOp::Shl,
                _ => break,
            };

            let (lbp, rbp) = infix_binding_power(op);
            if lbp < min_bp {
                break;
            }

            self.skip_newlines();
            self.advance(); // consume operator

            let rhs = self.parse_expr(rbp)?;
            let span = Span::new(lhs.span.start, rhs.span.end);
            lhs = Spanned::new(
                Expr::BinOp {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            );
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Spanned<Expr>, CompileError> {
        self.skip_newlines();
        let tok = self.peek().ok_or_else(|| {
            CompileError::syntax("unexpected end of file in expression", self.eof_span())
        })?;

        match &tok.node {
            Token::IntLit(_) => {
                let tok = self.advance().expect("token should exist after peek");
                let Token::IntLit(n) = &tok.node else { unreachable!() };
                Ok(Spanned::new(Expr::IntLit(*n), tok.span))
            }
            Token::FloatLit(_) => {
                let tok = self.advance().expect("token should exist after peek");
                let Token::FloatLit(n) = &tok.node else { unreachable!() };
                Ok(Spanned::new(Expr::FloatLit(*n), tok.span))
            }
            Token::True => {
                let tok = self.advance().expect("token should exist after peek");
                Ok(Spanned::new(Expr::BoolLit(true), tok.span))
            }
            Token::False => {
                let tok = self.advance().expect("token should exist after peek");
                Ok(Spanned::new(Expr::BoolLit(false), tok.span))
            }
            Token::FStringLit(_) => {
                // F-strings always support interpolation
                let tok = self.advance().expect("token should exist after peek");
                let Token::FStringLit(s) = &tok.node else { unreachable!() };
                let s = s.clone();
                let span = tok.span;
                if s.contains('{') || s.contains('}') {
                    self.parse_string_interp(&s, span)
                } else {
                    // Even without braces, it's still a valid f-string (just no interpolation)
                    Ok(Spanned::new(Expr::StringLit(s), span))
                }
            }
            Token::StringLit(_) => {
                let tok = self.advance().expect("token should exist after peek");
                let Token::StringLit(s) = &tok.node else { unreachable!() };
                let s = s.clone();
                let span = tok.span;
                if s.contains('{') || s.contains('}') {
                    self.parse_string_interp(&s, span)
                } else {
                    Ok(Spanned::new(Expr::StringLit(s), span))
                }
            }
            Token::Ident => {
                let ident = self.expect_ident()?;
                self.parse_expr_after_ident(ident)
            }
            Token::SelfVal => {
                let tok = self.advance().expect("token should exist after peek");
                Ok(Spanned::new(Expr::Ident("self".to_string()), tok.span))
            }
            Token::LParen => {
                if self.is_closure_ahead() {
                    self.parse_closure()
                } else {
                    self.advance(); // consume '('
                    let old_restrict = self.restrict_struct_lit;
                    self.restrict_struct_lit = false;
                    let expr = self.parse_expr(0)?;
                    self.restrict_struct_lit = old_restrict;
                    self.expect(&Token::RParen)?;
                    Ok(expr)
                }
            }
            Token::Minus => {
                let tok = self.advance().expect("token should exist after peek");
                let start = tok.span.start;
                // Use parse_expr(21) so that postfix ops (., [], !, ?, as, catch)
                // bind tighter than prefix unary operators.
                // 21 is above all infix operators (max is Mul/Div/Mod at 19-20).
                let operand = self.parse_expr(21)?;
                let end = operand.span.end;
                Ok(Spanned::new(
                    Expr::UnaryOp { op: UnaryOp::Neg, operand: Box::new(operand) },
                    Span::new(start, end),
                ))
            }
            Token::Bang => {
                let tok = self.advance().expect("token should exist after peek");
                let start = tok.span.start;
                let operand = self.parse_expr(21)?;
                let end = operand.span.end;
                Ok(Spanned::new(
                    Expr::UnaryOp { op: UnaryOp::Not, operand: Box::new(operand) },
                    Span::new(start, end),
                ))
            }
            Token::Tilde => {
                let tok = self.advance().expect("token should exist after peek");
                let start = tok.span.start;
                let operand = self.parse_expr(21)?;
                let end = operand.span.end;
                Ok(Spanned::new(
                    Expr::UnaryOp { op: UnaryOp::BitNot, operand: Box::new(operand) },
                    Span::new(start, end),
                ))
            }
            Token::LBracket => {
                let tok = self.advance().expect("token should exist after peek");
                let start = tok.span.start;
                self.skip_newlines();
                let elements = self.parse_comma_list(&Token::RBracket, true, |p| p.parse_expr(0))?;
                let close = self.expect(&Token::RBracket)?;
                let end = close.span.end;
                Ok(Spanned::new(Expr::ArrayLit { elements }, Span::new(start, end)))
            }
            Token::Spawn => {
                let spawn_tok = self.advance().expect("token should exist after peek");
                let start = spawn_tok.span.start;
                // Parse the first identifier (or `self`)
                let first = match self.peek_raw() {
                    Some(tok) if matches!(tok.node, Token::SelfVal) => {
                        let tok = self.advance().expect("token should exist");
                        Spanned::new("self".to_string(), tok.span)
                    }
                    _ => self.expect_ident()?,
                };
                // Check what follows: `(` means direct call, `.` means chain
                match self.peek() {
                    Some(tok) if matches!(tok.node, Token::LParen) => {
                        // spawn func(args)
                        self.advance(); // consume '('
                        self.skip_newlines();
                        let args = self.parse_comma_list(&Token::RParen, true, |p| p.parse_expr(0))?;
                        let close = self.expect(&Token::RParen)?;
                        let call_span = Span::new(first.span.start, close.span.end);
                        let call = Expr::Call { name: first, args, type_args: vec![], target_id: None };
                        Ok(Spanned::new(
                            Expr::Spawn { call: Box::new(Spanned::new(call, call_span)) },
                            Span::new(start, close.span.end),
                        ))
                    }
                    Some(tok) if matches!(tok.node, Token::Dot) => {
                        // spawn obj.field...field.method(args)
                        let mut object: Spanned<Expr> = Spanned::new(
                            Expr::Ident(first.node.clone()),
                            first.span,
                        );
                        loop {
                            self.advance(); // consume '.'
                            let member = self.expect_ident()?;
                            // Check if this member is followed by `(` (method call) or `.` (field access)
                            match self.peek() {
                                Some(tok) if matches!(tok.node, Token::LParen) => {
                                    // This is the terminal method call
                                    self.advance(); // consume '('
                                    self.skip_newlines();
                                    let args = self.parse_comma_list(&Token::RParen, true, |p| p.parse_expr(0))?;
                                    let close = self.expect(&Token::RParen)?;
                                    let call_span = Span::new(first.span.start, close.span.end);
                                    let call = Expr::MethodCall {
                                        object: Box::new(object),
                                        method: member,
                                        args,
                                    };
                                    return Ok(Spanned::new(
                                        Expr::Spawn { call: Box::new(Spanned::new(call, call_span)) },
                                        Span::new(start, close.span.end),
                                    ));
                                }
                                Some(tok) if matches!(tok.node, Token::Dot) => {
                                    // Field access, continue chain
                                    let fa_span = Span::new(object.span.start, member.span.end);
                                    object = Spanned::new(
                                        Expr::FieldAccess { object: Box::new(object), field: member },
                                        fa_span,
                                    );
                                }
                                _ => {
                                    return Err(CompileError::syntax(
                                        "expected '(' or '.' after identifier in spawn expression".to_string(),
                                        member.span,
                                    ));
                                }
                            }
                        }
                    }
                    _ => {
                        Err(CompileError::syntax(
                            "expected '(' or '.' after identifier in spawn expression".to_string(),
                            first.span,
                        ))
                    }
                }
            }
            Token::None => {
                let tok = self.advance().expect("token should exist after peek");
                Ok(Spanned::new(Expr::NoneLit, tok.span))
            }
            _ => Err(CompileError::syntax(
                format!("unexpected token {} in expression", tok.node),
                tok.span,
            )),
        }
    }

    /// Continue parsing an expression that started with an identifier (handles calls, struct literals, and plain ident).
    fn parse_expr_after_ident(&mut self, ident: Spanned<String>) -> Result<Spanned<Expr>, CompileError> {
        // Check for static trait call: TraitName::method<TypeArgs>(args)
        // We've already consumed the trait name, so check if `::` follows
        if self.peek().is_some()
            && matches!(self.peek().expect("token should exist after is_some check").node, Token::DoubleColon)
        {
            // Simplified check - just verify this is actually a trait call by looking ahead
            // Pattern after :: should be: Ident <
            let mut i = self.pos;
            // skip newlines to find ::
            while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
                i += 1;
            }
            if i < self.tokens.len() && matches!(self.tokens[i].node, Token::DoubleColon) {
                i += 1;
                // skip newlines after ::
                while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
                    i += 1;
                }
                // Check for Ident followed by <
                if i < self.tokens.len() && matches!(self.tokens[i].node, Token::Ident) {
                    i += 1;
                    while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
                        i += 1;
                    }
                    if i < self.tokens.len() && matches!(self.tokens[i].node, Token::Lt) {
                        return self.parse_static_trait_call(ident);
                    }
                }
            }
        }
        // Check for explicit type args on function call: ident<type_args>(args)
        if self.peek().is_some()
            && matches!(self.peek().expect("token should exist after is_some check").node, Token::Lt)
            && self.is_generic_call_ahead()
        {
            let type_args = self.parse_type_arg_list()?;
            self.expect(&Token::LParen)?;
            self.skip_newlines();
            let args = self.parse_comma_list(&Token::RParen, true, |p| p.parse_expr(0))?;
            let close = self.expect(&Token::RParen)?;
            let span = Span::new(ident.span.start, close.span.end);
            return Ok(Spanned::new(Expr::Call { name: ident, args, type_args, target_id: None }, span));
        }
        // Check for function call
        if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LParen) {
            self.advance(); // consume '('
            self.skip_newlines();
            let args = self.parse_comma_list(&Token::RParen, true, |p| p.parse_expr(0))?;
            let close = self.expect(&Token::RParen)?;
            let span = Span::new(ident.span.start, close.span.end);
            Ok(Spanned::new(Expr::Call { name: ident, args, type_args: vec![], target_id: None }, span))
        } else if !self.restrict_struct_lit && self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace) {
            // Check if this looks like a struct literal: Ident { field: value, ... }
            // We need to distinguish from a block. A struct literal has `ident : expr` inside.
            // Use a lookahead: after `{`, if we see `ident :` it's a struct literal.
            if self.is_struct_lit_ahead() {
                self.advance(); // consume '{'
                let (fields, close_end) = self.parse_field_list()?;
                let span = Span::new(ident.span.start, close_end);
                Ok(Spanned::new(Expr::StructLit { name: ident, type_args: vec![], fields, target_id: None }, span))
            } else {
                Ok(Spanned::new(Expr::Ident(ident.node.clone()), ident.span))
            }
        } else if !self.restrict_struct_lit
            && (ident.node == "Map" || ident.node == "Set")
            && self.peek().is_some()
            && matches!(self.peek().expect("token should exist after is_some check").node, Token::Lt)
        {
            // Map<K, V> { ... } or Set<T> { ... }
            let start = ident.span.start;
            let type_args = self.parse_type_arg_list()?;
            self.expect(&Token::LBrace)?;
            self.skip_newlines();

            if ident.node == "Map" {
                if type_args.len() != 2 {
                    return Err(CompileError::syntax(
                        format!("Map expects 2 type arguments, got {}", type_args.len()),
                        ident.span,
                    ));
                }
                let key_type = type_args[0].clone();
                let value_type = type_args[1].clone();
                let entries = self.parse_comma_list(&Token::RBrace, false, |p| {
                    let key_expr = p.parse_expr(0)?;
                    p.expect(&Token::Colon)?;
                    let val_expr = p.parse_expr(0)?;
                    Ok((key_expr, val_expr))
                })?;
                let close = self.expect(&Token::RBrace)?;
                let span = Span::new(start, close.span.end);
                Ok(Spanned::new(Expr::MapLit { key_type, value_type, entries }, span))
            } else {
                // Set
                if type_args.len() != 1 {
                    return Err(CompileError::syntax(
                        format!("Set expects 1 type argument, got {}", type_args.len()),
                        ident.span,
                    ));
                }
                let elem_type = type_args[0].clone();
                let elements = self.parse_comma_list(&Token::RBrace, false, |p| p.parse_expr(0))?;
                let close = self.expect(&Token::RBrace)?;
                let span = Span::new(start, close.span.end);
                Ok(Spanned::new(Expr::SetLit { elem_type, elements }, span))
            }
        } else if !self.restrict_struct_lit
            && self.peek().is_some()
            && matches!(self.peek().expect("token should exist after is_some check").node, Token::Lt)
            && self.is_generic_struct_lit_ahead()
        {
            // Generic struct literal: Ident<type_args> { field: value, ... }
            let start = ident.span.start;
            let type_args = self.parse_type_arg_list()?;
            self.advance(); // consume '{'
            let (fields, close_end) = self.parse_field_list()?;
            let span = Span::new(start, close_end);
            Ok(Spanned::new(Expr::StructLit { name: ident, type_args, fields, target_id: None }, span))
        } else {
            Ok(Spanned::new(Expr::Ident(ident.node.clone()), ident.span))
        }
    }

    /// Parse static trait call: TraitName::method<TypeArgs>(args)
    fn parse_static_trait_call(&mut self, trait_name: Spanned<String>) -> Result<Spanned<Expr>, CompileError> {
        let start = trait_name.span.start;
        self.expect(&Token::DoubleColon)?;
        let method_name = self.expect_ident()?;

        // Parse type arguments
        let type_args = self.parse_type_arg_list()?;

        // Parse function arguments
        self.expect(&Token::LParen)?;
        self.skip_newlines();
        let args = self.parse_comma_list(&Token::RParen, true, |p| p.parse_expr(0))?;
        let close = self.expect(&Token::RParen)?;

        let span = Span::new(start, close.span.end);
        Ok(Spanned::new(
            Expr::StaticTraitCall {
                trait_name,
                method_name,
                type_args,
                args,
            },
            span,
        ))
    }

    fn is_at_end(&self) -> bool {
        let mut i = self.pos;
        while i < self.tokens.len() {
            if !matches!(self.tokens[i].node, Token::Newline) {
                return false;
            }
            i += 1;
        }
        true
    }

    fn parse_string_interp(&self, raw: &str, span: crate::span::Span) -> Result<Spanned<Expr>, CompileError> {
        let mut parts: Vec<StringInterpPart> = Vec::new();
        let mut lit_buf = String::new();
        let mut chars = raw.char_indices().peekable();

        while let Some(&(_, ch)) = chars.peek() {
            if ch == '{' {
                chars.next();
                // Check for escaped {{
                if chars.peek().is_some_and(|&(_, c)| c == '{') {
                    chars.next();
                    lit_buf.push('{');
                } else {
                    // Flush literal buffer
                    if !lit_buf.is_empty() {
                        parts.push(StringInterpPart::Lit(std::mem::take(&mut lit_buf)));
                    }
                    // Collect expression chars until matching }
                    let mut expr_str = String::new();
                    let mut depth = 1;
                    loop {
                        match chars.next() {
                            None => {
                                return Err(CompileError::syntax(
                                    "unterminated interpolation expression",
                                    span,
                                ));
                            }
                            Some((_, '{')) => {
                                depth += 1;
                                expr_str.push('{');
                            }
                            Some((_, '}')) => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                expr_str.push('}');
                            }
                            Some((_, c)) => {
                                expr_str.push(c);
                            }
                        }
                    }
                    // Sub-parse the expression
                    let tokens = crate::lexer::lex(&expr_str)?;
                    let mut sub_parser = Parser::new(&tokens, &expr_str);
                    let expr = sub_parser.parse_expr(0)?;
                    if !sub_parser.is_at_end() {
                        return Err(CompileError::syntax(
                            "unexpected tokens in interpolation expression",
                            span,
                        ));
                    }
                    parts.push(StringInterpPart::Expr(expr));
                }
            } else if ch == '}' {
                chars.next();
                // Check for escaped }}
                if chars.peek().is_some_and(|&(_, c)| c == '}') {
                    chars.next();
                    lit_buf.push('}');
                } else {
                    return Err(CompileError::syntax(
                        "unexpected '}' in string literal (use '}}' for literal brace)",
                        span,
                    ));
                }
            } else {
                chars.next();
                lit_buf.push(ch);
            }
        }

        // Flush remaining literal
        if !lit_buf.is_empty() {
            parts.push(StringInterpPart::Lit(std::mem::take(&mut lit_buf)));
        }

        // Optimization: single literal → plain StringLit
        if parts.len() == 1 && let StringInterpPart::Lit(s) = &parts[0] {
            return Ok(Spanned::new(Expr::StringLit(s.clone()), span));
        }

        Ok(Spanned::new(Expr::StringInterp { parts }, span))
    }

    /// Lookahead to determine if `(` starts a closure expression.
    /// A closure starts with `(` followed by either:
    /// - `)` then `=>` (zero-param closure)
    /// - `ident` then `:` (typed params — closure)
    fn is_closure_ahead(&self) -> bool {
        // We're positioned such that peek() returns LParen.
        // Find the position of the LParen in the raw token stream.
        let mut i = self.pos;
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::LParen) {
            return false;
        }
        i += 1; // skip past '('
        // Skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() {
            return false;
        }
        // Case 1: `() =>` — zero-param closure
        if matches!(self.tokens[i].node, Token::RParen) {
            i += 1;
            while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
                i += 1;
            }
            return i < self.tokens.len() && matches!(self.tokens[i].node, Token::FatArrow);
        }
        // Case 2: `(ident :` — closure with typed params
        if matches!(self.tokens[i].node, Token::Ident) {
            i += 1;
            while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
                i += 1;
            }
            return i < self.tokens.len() && matches!(self.tokens[i].node, Token::Colon);
        }
        false
    }

    /// Parse a closure expression: `(params) => body` or `(params) return_type => body`
    fn parse_closure(&mut self) -> Result<Spanned<Expr>, CompileError> {
        let open = self.expect(&Token::LParen)?;
        let start = open.span.start;

        // Parse params
        let params = self.parse_comma_list(&Token::RParen, true, |p| {
            let pname = p.expect_ident()?;
            p.expect(&Token::Colon)?;
            let pty = p.parse_type()?;
            Ok(Param { id: Uuid::new_v4(), name: pname, ty: pty, is_mut: false })
        })?;
        self.expect(&Token::RParen)?;

        // Optional return type: if next non-newline token is NOT `=>`, parse a type first
        let return_type = if self.peek().is_some() && !matches!(self.peek().expect("token should exist after is_some check").node, Token::FatArrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&Token::FatArrow)?;

        // Body: either a block `{ ... }` or a single expression (desugared to return stmt)
        let body = if self.peek().is_some() && matches!(self.peek().expect("token should exist after is_some check").node, Token::LBrace) {
            self.parse_block()?
        } else {
            let expr = self.parse_expr(0)?;
            let span = expr.span;
            Spanned::new(
                Block { stmts: vec![Spanned::new(Stmt::Return(Some(expr)), span)] },
                span,
            )
        };

        let end = body.span.end;
        Ok(Spanned::new(
            Expr::Closure { params, return_type, body },
            Span::new(start, end),
        ))
    }

    /// Lookahead to determine if `<...> {` starts a generic struct literal.
    /// We're positioned at `<`. Count balanced `<>`s; if they close and next non-newline is `{`
    /// containing `ident:`, return true.
    fn is_generic_struct_lit_ahead(&self) -> bool {
        let mut i = self.pos;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Lt) {
            return false;
        }
        i += 1;
        let mut depth = 1;
        while i < self.tokens.len() && depth > 0 {
            match &self.tokens[i].node {
                Token::Lt => depth += 1,
                Token::Gt => depth -= 1,
                Token::GtEq => {
                    // `>=` could be `> =` in some contexts but not here
                    return false;
                }
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            return false;
        }
        // Skip newlines after `>`
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        // Must be followed by `{` and then `ident :`
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::LBrace) {
            return false;
        }
        // Now check if contents look like struct fields: ident :
        i += 1;
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Ident) {
            return false;
        }
        i += 1;
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        i < self.tokens.len() && matches!(self.tokens[i].node, Token::Colon)
    }

    /// Lookahead from current position (at `<`) to see if balanced `<...>` followed by `.`.
    /// Lookahead to determine if `<` starts explicit type args for a function call: `ident<...>(`
    fn is_generic_call_ahead(&self) -> bool {
        let mut i = self.pos;
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Lt) {
            return false;
        }
        i += 1;
        let mut depth = 1;
        while i < self.tokens.len() && depth > 0 {
            match &self.tokens[i].node {
                Token::Lt => depth += 1,
                Token::Gt => depth -= 1,
                Token::GtEq => return false,
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            return false;
        }
        // Must be followed by `(`
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        i < self.tokens.len() && matches!(self.tokens[i].node, Token::LParen)
    }

    fn is_generic_enum_expr_ahead(&self) -> bool {
        let mut i = self.pos;
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Lt) {
            return false;
        }
        i += 1;
        let mut depth = 1;
        while i < self.tokens.len() && depth > 0 {
            match &self.tokens[i].node {
                Token::Lt => depth += 1,
                Token::Gt => depth -= 1,
                Token::GtEq => return false,
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            return false;
        }
        // Must be followed by `.`
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        i < self.tokens.len() && matches!(self.tokens[i].node, Token::Dot)
    }

    /// Lookahead to determine if `{ ... }` is a struct literal (contains `ident :`)
    fn is_struct_lit_ahead(&self) -> bool {
        // We're positioned at `{`. Look past it for `ident :` or `}`
        let mut i = self.pos + 1;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() {
            return false;
        }
        // Check for empty struct literal: Foo {}
        if matches!(self.tokens[i].node, Token::RBrace) {
            return true;
        }
        // Must be an identifier
        if !matches!(self.tokens[i].node, Token::Ident) {
            return false;
        }
        i += 1;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() {
            return false;
        }
        // Must be a colon
        matches!(self.tokens[i].node, Token::Colon)
    }

    /// Lookahead to determine if we have a static trait call: `TraitName::method<TypeArgs>`
    fn is_static_trait_call_ahead(&self) -> bool {
        // Pattern: Ident :: Ident <
        let mut i = self.pos;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Ident) {
            return false;
        }
        i += 1;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::DoubleColon) {
            return false;
        }
        i += 1;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() || !matches!(self.tokens[i].node, Token::Ident) {
            return false;
        }
        i += 1;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        // Must be followed by `<` for type args
        i < self.tokens.len() && matches!(self.tokens[i].node, Token::Lt)
    }
}

fn infix_binding_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Or => (1, 2),
        BinOp::And => (3, 4),
        BinOp::BitOr => (5, 6),
        BinOp::BitXor => (7, 8),
        BinOp::BitAnd => (9, 10),
        BinOp::Eq | BinOp::Neq => (11, 12),
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => (13, 14),
        BinOp::Shl | BinOp::Shr => (15, 16),
        BinOp::Add | BinOp::Sub => (17, 18),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (19, 20),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse(src: &str) -> Program {
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        parser.parse_program().unwrap()
    }

    #[test]
    fn parse_empty_main() {
        let prog = parse("fn main() { }");
        assert_eq!(prog.functions.len(), 1);
        assert_eq!(prog.functions[0].node.name.node, "main");
        assert!(prog.functions[0].node.params.is_empty());
        assert!(prog.functions[0].node.return_type.is_none());
    }

    #[test]
    fn parse_function_with_params() {
        let prog = parse("fn add(a: int, b: int) int {\n    return a + b\n}");
        let f = &prog.functions[0].node;
        assert_eq!(f.name.node, "add");
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].name.node, "a");
        assert_eq!(f.params[1].name.node, "b");
        assert!(f.return_type.is_some());
    }

    #[test]
    fn parse_let_and_call() {
        let prog = parse("fn main() {\n    let x = add(1, 2)\n}");
        let f = &prog.functions[0].node;
        assert_eq!(f.body.node.stmts.len(), 1);
        match &f.body.node.stmts[0].node {
            Stmt::Let { name, value, .. } => {
                assert_eq!(name.node, "x");
                assert!(matches!(value.node, Expr::Call { .. }));
            }
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_operator_precedence() {
        let prog = parse("fn main() {\n    let x = 1 + 2 * 3\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                // Should be Add(1, Mul(2, 3))
                match &value.node {
                    Expr::BinOp { op: BinOp::Add, rhs, .. } => {
                        assert!(matches!(rhs.node, Expr::BinOp { op: BinOp::Mul, .. }));
                    }
                    _ => panic!("expected binop"),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_if_else() {
        let prog = parse("fn main() {\n    if true {\n        let x = 1\n    } else {\n        let x = 2\n    }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::If { else_block, .. } => {
                assert!(else_block.is_some());
            }
            _ => panic!("expected if statement"),
        }
    }

    #[test]
    fn parse_while_loop() {
        let prog = parse("fn main() {\n    while true {\n        let x = 1\n    }\n}");
        let f = &prog.functions[0].node;
        assert!(matches!(f.body.node.stmts[0].node, Stmt::While { .. }));
    }

    #[test]
    fn parse_multiple_functions() {
        let prog = parse("fn foo() {\n}\n\nfn bar() {\n}");
        assert_eq!(prog.functions.len(), 2);
    }

    #[test]
    fn parse_class_decl() {
        let prog = parse("class Point {\n    x: int\n    y: int\n}");
        assert_eq!(prog.classes.len(), 1);
        let c = &prog.classes[0].node;
        assert_eq!(c.name.node, "Point");
        assert_eq!(c.fields.len(), 2);
        assert_eq!(c.fields[0].name.node, "x");
        assert_eq!(c.fields[1].name.node, "y");
        assert!(c.methods.is_empty());
    }

    #[test]
    fn parse_class_with_method() {
        let prog = parse("class Point {\n    x: int\n    y: int\n\n    fn get_x(self) int {\n        return self.x\n    }\n}");
        let c = &prog.classes[0].node;
        assert_eq!(c.fields.len(), 2);
        assert_eq!(c.methods.len(), 1);
        assert_eq!(c.methods[0].node.name.node, "get_x");
        assert_eq!(c.methods[0].node.params[0].name.node, "self");
    }

    #[test]
    fn parse_struct_literal() {
        let prog = parse("fn main() {\n    let p = Point { x: 1, y: 2 }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::StructLit { name, fields, .. } => {
                        assert_eq!(name.node, "Point");
                        assert_eq!(fields.len(), 2);
                        assert_eq!(fields[0].0.node, "x");
                        assert_eq!(fields[1].0.node, "y");
                    }
                    _ => panic!("expected struct literal, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_field_access() {
        let prog = parse("fn main() {\n    let x = p.x\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::FieldAccess { field, .. } => {
                        assert_eq!(field.node, "x");
                    }
                    _ => panic!("expected field access"),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_method_call() {
        let prog = parse("fn main() {\n    let x = p.get_x()\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::MethodCall { method, args, .. } => {
                        assert_eq!(method.node, "get_x");
                        assert!(args.is_empty());
                    }
                    _ => panic!("expected method call"),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_field_assign() {
        let prog = parse("fn main() {\n    p.x = 42\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::FieldAssign { field, .. } => {
                assert_eq!(field.node, "x");
            }
            _ => panic!("expected field assign, got {:?}", f.body.node.stmts[0].node),
        }
    }

    #[test]
    fn parse_self_field_assign() {
        let prog = parse("class Foo {\n    x: int\n\n    fn set_x(self, v: int) {\n        self.x = v\n    }\n}");
        let m = &prog.classes[0].node.methods[0].node;
        match &m.body.node.stmts[0].node {
            Stmt::FieldAssign { field, .. } => {
                assert_eq!(field.node, "x");
            }
            _ => panic!("expected field assign"),
        }
    }

    #[test]
    fn parse_import() {
        let prog = parse("import math\n\nfn main() { }");
        assert_eq!(prog.imports.len(), 1);
        assert_eq!(prog.imports[0].node.binding_name(), "math");
        assert_eq!(prog.imports[0].node.full_path(), "math");
        assert_eq!(prog.functions.len(), 1);
    }

    #[test]
    fn parse_dotted_import() {
        let prog = parse("import std.io.fs\n\nfn main() { }");
        assert_eq!(prog.imports.len(), 1);
        assert_eq!(prog.imports[0].node.path.len(), 3);
        assert_eq!(prog.imports[0].node.binding_name(), "fs");
        assert_eq!(prog.imports[0].node.full_path(), "std.io.fs");
    }

    #[test]
    fn parse_import_alias() {
        let prog = parse("import std.io as io\n\nfn main() { }");
        assert_eq!(prog.imports.len(), 1);
        assert_eq!(prog.imports[0].node.binding_name(), "io");
        assert_eq!(prog.imports[0].node.full_path(), "std.io");
    }

    #[test]
    fn parse_extern_fn_decl() {
        let prog = parse("extern fn __pluto_print(s: string)\n\nfn main() { }");
        assert_eq!(prog.extern_fns.len(), 1);
        assert_eq!(prog.extern_fns[0].node.name.node, "__pluto_print");
        assert_eq!(prog.extern_fns[0].node.params.len(), 1);
        assert!(prog.extern_fns[0].node.return_type.is_none());
        assert!(!prog.extern_fns[0].node.is_pub);
    }

    #[test]
    fn parse_pub_extern_fn_with_return() {
        let prog = parse("pub extern fn __read(path: string) string\n\nfn main() { }");
        assert_eq!(prog.extern_fns.len(), 1);
        assert!(prog.extern_fns[0].node.is_pub);
        assert!(prog.extern_fns[0].node.return_type.is_some());
    }

    #[test]
    fn parse_pub_function() {
        let prog = parse("pub fn add(a: int, b: int) int {\n    return a + b\n}");
        assert!(prog.functions[0].node.is_pub);
    }

    #[test]
    fn parse_non_pub_function() {
        let prog = parse("fn add(a: int, b: int) int {\n    return a + b\n}");
        assert!(!prog.functions[0].node.is_pub);
    }

    #[test]
    fn parse_pub_class() {
        let prog = parse("pub class Point {\n    x: int\n}");
        assert!(prog.classes[0].node.is_pub);
    }

    #[test]
    fn parse_qualified_type() {
        let prog = parse("fn foo(p: math.Point) { }");
        let f = &prog.functions[0].node;
        match &f.params[0].ty.node {
            TypeExpr::Qualified { module, name } => {
                assert_eq!(module, "math");
                assert_eq!(name, "Point");
            }
            _ => panic!("expected qualified type"),
        }
    }

    #[test]
    fn parse_for_loop() {
        let prog = parse("fn main() {\n    for x in arr {\n        print(x)\n    }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::For { var, .. } => {
                assert_eq!(var.node, "x");
            }
            _ => panic!("expected for statement"),
        }
    }

    #[test]
    fn parse_qualified_struct_lit() {
        let prog = parse("fn main() {\n    let p = math.Point { x: 1, y: 2 }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::StructLit { name, fields, .. } => {
                        assert_eq!(name.node, "math.Point");
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("expected struct literal, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_enum_decl_unit() {
        let prog = parse("enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn main() { }");
        assert_eq!(prog.enums.len(), 1);
        let e = &prog.enums[0].node;
        assert_eq!(e.name.node, "Color");
        assert_eq!(e.variants.len(), 3);
        assert_eq!(e.variants[0].name.node, "Red");
        assert!(e.variants[0].fields.is_empty());
    }

    #[test]
    fn parse_enum_decl_data() {
        let prog = parse("enum Status {\n    Active\n    Suspended { reason: string }\n}\n\nfn main() { }");
        let e = &prog.enums[0].node;
        assert_eq!(e.variants.len(), 2);
        assert!(e.variants[0].fields.is_empty());
        assert_eq!(e.variants[1].fields.len(), 1);
        assert_eq!(e.variants[1].fields[0].name.node, "reason");
    }

    #[test]
    fn parse_enum_unit_expr() {
        let prog = parse("enum Color {\n    Red\n}\n\nfn main() {\n    let c = Color.Red\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::EnumUnit { enum_name, variant, .. } => {
                        assert_eq!(enum_name.node, "Color");
                        assert_eq!(variant.node, "Red");
                    }
                    _ => panic!("expected EnumUnit, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_enum_data_expr() {
        let prog = parse("enum Status {\n    Suspended { reason: string }\n}\n\nfn main() {\n    let s = Status.Suspended { reason: \"banned\" }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::EnumData { enum_name, variant, fields, .. } => {
                        assert_eq!(enum_name.node, "Status");
                        assert_eq!(variant.node, "Suspended");
                        assert_eq!(fields.len(), 1);
                        assert_eq!(fields[0].0.node, "reason");
                    }
                    _ => panic!("expected EnumData, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_match_unit_arm() {
        let prog = parse("enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            let x = 1\n        }\n        Color.Blue {\n            let x = 2\n        }\n    }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[1].node {
            Stmt::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert_eq!(arms[0].enum_name.node, "Color");
                assert_eq!(arms[0].variant_name.node, "Red");
                assert!(arms[0].bindings.is_empty());
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_match_binding_arm() {
        let prog = parse("enum Status {\n    Active\n    Suspended { reason: string }\n}\n\nfn main() {\n    let s = Status.Active\n    match s {\n        Status.Active {\n            let x = 1\n        }\n        Status.Suspended { reason } {\n            print(reason)\n        }\n    }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[1].node {
            Stmt::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert_eq!(arms[1].variant_name.node, "Suspended");
                assert_eq!(arms[1].bindings.len(), 1);
                assert_eq!(arms[1].bindings[0].0.node, "reason");
                assert!(arms[1].bindings[0].1.is_none());
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_string_interpolation() {
        let prog = parse("fn main() {\n    let x = \"hello {name}\"\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::StringInterp { parts } => {
                        assert_eq!(parts.len(), 2);
                        assert!(matches!(&parts[0], StringInterpPart::Lit(s) if s == "hello "));
                        assert!(matches!(&parts[1], StringInterpPart::Expr(_)));
                    }
                    _ => panic!("expected string interp, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_string_interp_escaped_braces() {
        let prog = parse("fn main() {\n    let x = \"{{x}}\"\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::StringLit(s) => {
                        assert_eq!(s, "{x}");
                    }
                    _ => panic!("expected string lit, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_closure_no_params() {
        let prog = parse("fn main() {\n    let f = () => 42\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::Closure { params, return_type, body } => {
                        assert!(params.is_empty());
                        assert!(return_type.is_none());
                        assert_eq!(body.node.stmts.len(), 1);
                        assert!(matches!(&body.node.stmts[0].node, Stmt::Return(Some(_))));
                    }
                    _ => panic!("expected closure, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_closure_single_param() {
        let prog = parse("fn main() {\n    let f = (x: int) => x + 1\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::Closure { params, return_type, .. } => {
                        assert_eq!(params.len(), 1);
                        assert_eq!(params[0].name.node, "x");
                        assert!(return_type.is_none());
                    }
                    _ => panic!("expected closure, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_closure_multi_params() {
        let prog = parse("fn main() {\n    let f = (x: int, y: int) => x + y\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::Closure { params, .. } => {
                        assert_eq!(params.len(), 2);
                        assert_eq!(params[0].name.node, "x");
                        assert_eq!(params[1].name.node, "y");
                    }
                    _ => panic!("expected closure, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_closure_block_body() {
        let prog = parse("fn main() {\n    let f = (x: int) => {\n        return x + 1\n    }\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::Closure { params, body, .. } => {
                        assert_eq!(params.len(), 1);
                        assert_eq!(body.node.stmts.len(), 1);
                        assert!(matches!(&body.node.stmts[0].node, Stmt::Return(Some(_))));
                    }
                    _ => panic!("expected closure, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_closure_with_return_type() {
        let prog = parse("fn main() {\n    let f = (x: int) int => x + 1\n}");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                match &value.node {
                    Expr::Closure { params, return_type, .. } => {
                        assert_eq!(params.len(), 1);
                        assert!(return_type.is_some());
                        assert!(matches!(&return_type.as_ref().unwrap().node, TypeExpr::Named(n) if n == "int"));
                    }
                    _ => panic!("expected closure, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_fn_type() {
        let prog = parse("fn apply(f: fn(int, int) int, x: int) int {\n    return f(x, x)\n}");
        let f = &prog.functions[0].node;
        match &f.params[0].ty.node {
            TypeExpr::Fn { params, return_type } => {
                assert_eq!(params.len(), 2);
                assert!(matches!(&params[0].node, TypeExpr::Named(n) if n == "int"));
                assert!(matches!(&params[1].node, TypeExpr::Named(n) if n == "int"));
                assert!(matches!(&return_type.node, TypeExpr::Named(n) if n == "int"));
            }
            _ => panic!("expected fn type, got {:?}", f.params[0].ty.node),
        }
    }

    #[test]
    fn parse_fn_type_void() {
        let prog = parse("fn apply(f: fn(int)) {\n}");
        let f = &prog.functions[0].node;
        match &f.params[0].ty.node {
            TypeExpr::Fn { params, return_type } => {
                assert_eq!(params.len(), 1);
                assert!(matches!(&return_type.node, TypeExpr::Named(n) if n == "void"));
            }
            _ => panic!("expected fn type, got {:?}", f.params[0].ty.node),
        }
    }

    #[test]
    fn parse_app_decl_basic() {
        let prog = parse("app MyApp {\n    fn main(self) {\n    }\n}");
        let app = prog.app.as_ref().unwrap();
        assert_eq!(app.node.name.node, "MyApp");
        assert!(app.node.inject_fields.is_empty());
        assert_eq!(app.node.methods.len(), 1);
        assert_eq!(app.node.methods[0].node.name.node, "main");
    }

    #[test]
    fn parse_app_with_bracket_deps() {
        let prog = parse("class Database {\n}\n\napp MyApp[db: Database] {\n    fn main(self) {\n    }\n}");
        let app = prog.app.as_ref().unwrap();
        assert_eq!(app.node.name.node, "MyApp");
        assert_eq!(app.node.inject_fields.len(), 1);
        assert_eq!(app.node.inject_fields[0].name.node, "db");
        assert!(app.node.inject_fields[0].is_injected);
    }

    #[test]
    fn parse_class_with_bracket_deps() {
        let prog = parse("class Database {\n}\n\nclass UserService[db: Database] {\n    fn query(self) {\n    }\n}");
        let c = &prog.classes[1].node;
        assert_eq!(c.name.node, "UserService");
        assert_eq!(c.fields.len(), 1);
        assert_eq!(c.fields[0].name.node, "db");
        assert!(c.fields[0].is_injected);
    }

    #[test]
    fn parse_class_bracket_deps_and_regular_fields() {
        let prog = parse("class Database {\n}\n\nclass Service[db: Database] {\n    name: string\n\n    fn run(self) {\n    }\n}");
        let c = &prog.classes[1].node;
        assert_eq!(c.fields.len(), 2);
        assert_eq!(c.fields[0].name.node, "db");
        assert!(c.fields[0].is_injected);
        assert_eq!(c.fields[1].name.node, "name");
        assert!(!c.fields[1].is_injected);
    }

    // ==================== Generics ====================

    #[test]
    fn parse_generic_function() {
        let prog = parse("fn identity<T>(x: T) T {\n    return x\n}");
        assert_eq!(prog.functions.len(), 1);
        let f = &prog.functions[0].node;
        assert_eq!(f.name.node, "identity");
        assert_eq!(f.type_params.len(), 1);
        assert_eq!(f.type_params[0].node, "T");
        assert_eq!(f.params.len(), 1);
        assert!(matches!(&f.params[0].ty.node, TypeExpr::Named(n) if n == "T"));
        assert!(matches!(&f.return_type.as_ref().unwrap().node, TypeExpr::Named(n) if n == "T"));
    }

    #[test]
    fn parse_generic_function_two_params() {
        let prog = parse("fn swap<A, B>(a: A, b: B) A {\n    return a\n}");
        let f = &prog.functions[0].node;
        assert_eq!(f.type_params.len(), 2);
        assert_eq!(f.type_params[0].node, "A");
        assert_eq!(f.type_params[1].node, "B");
    }

    #[test]
    fn parse_generic_class() {
        let prog = parse("class Pair<A, B> {\n    first: A\n    second: B\n}");
        assert_eq!(prog.classes.len(), 1);
        let c = &prog.classes[0].node;
        assert_eq!(c.name.node, "Pair");
        assert_eq!(c.type_params.len(), 2);
        assert_eq!(c.type_params[0].node, "A");
        assert_eq!(c.type_params[1].node, "B");
        assert_eq!(c.fields.len(), 2);
        assert!(matches!(&c.fields[0].ty.node, TypeExpr::Named(n) if n == "A"));
        assert!(matches!(&c.fields[1].ty.node, TypeExpr::Named(n) if n == "B"));
    }

    #[test]
    fn parse_generic_enum() {
        let prog = parse("enum Option<T> {\n    Some { value: T }\n    None\n}");
        assert_eq!(prog.enums.len(), 1);
        let e = &prog.enums[0].node;
        assert_eq!(e.name.node, "Option");
        assert_eq!(e.type_params.len(), 1);
        assert_eq!(e.type_params[0].node, "T");
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].name.node, "Some");
        assert_eq!(e.variants[1].name.node, "None");
    }

    #[test]
    fn parse_generic_type_in_annotation() {
        let prog = parse("fn foo() {\n    let x: Pair<int, string> = 0\n}");
        let body = &prog.functions[0].node.body.node;
        if let Stmt::Let { ty: Some(ty), .. } = &body.stmts[0].node {
            match &ty.node {
                TypeExpr::Generic { name, type_args } => {
                    assert_eq!(name, "Pair");
                    assert_eq!(type_args.len(), 2);
                    assert!(matches!(&type_args[0].node, TypeExpr::Named(n) if n == "int"));
                    assert!(matches!(&type_args[1].node, TypeExpr::Named(n) if n == "string"));
                }
                other => panic!("expected TypeExpr::Generic, got {:?}", other),
            }
        } else {
            panic!("expected let with type annotation");
        }
    }

    #[test]
    fn parse_generic_struct_lit() {
        let prog = parse("class Pair<A, B> {\n    first: A\n    second: B\n}\n\nfn main() {\n    let p = Pair<int, string> { first: 1, second: \"hi\" }\n}");
        let body = &prog.functions[0].node.body.node;
        if let Stmt::Let { value, .. } = &body.stmts[0].node {
            match &value.node {
                Expr::StructLit { name, type_args, fields, .. } => {
                    assert_eq!(name.node, "Pair");
                    assert_eq!(type_args.len(), 2);
                    assert!(matches!(&type_args[0].node, TypeExpr::Named(n) if n == "int"));
                    assert!(matches!(&type_args[1].node, TypeExpr::Named(n) if n == "string"));
                    assert_eq!(fields.len(), 2);
                }
                other => panic!("expected StructLit, got {:?}", other),
            }
        } else {
            panic!("expected let");
        }
    }

    #[test]
    fn parse_generic_enum_unit() {
        let prog = parse("enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let n = Option<int>.None\n}");
        let body = &prog.functions[0].node.body.node;
        if let Stmt::Let { value, .. } = &body.stmts[0].node {
            match &value.node {
                Expr::EnumUnit { enum_name, variant, type_args, .. } => {
                    assert_eq!(enum_name.node, "Option");
                    assert_eq!(variant.node, "None");
                    assert_eq!(type_args.len(), 1);
                    assert!(matches!(&type_args[0].node, TypeExpr::Named(n) if n == "int"));
                }
                other => panic!("expected EnumUnit, got {:?}", other),
            }
        } else {
            panic!("expected let");
        }
    }

    #[test]
    fn parse_generic_enum_data() {
        let prog = parse("enum Option<T> {\n    Some { value: T }\n    None\n}\n\nfn main() {\n    let s = Option<int>.Some { value: 42 }\n}");
        let body = &prog.functions[0].node.body.node;
        if let Stmt::Let { value, .. } = &body.stmts[0].node {
            match &value.node {
                Expr::EnumData { enum_name, variant, fields, type_args, .. } => {
                    assert_eq!(enum_name.node, "Option");
                    assert_eq!(variant.node, "Some");
                    assert_eq!(type_args.len(), 1);
                    assert!(matches!(&type_args[0].node, TypeExpr::Named(n) if n == "int"));
                    assert_eq!(fields.len(), 1);
                }
                other => panic!("expected EnumData, got {:?}", other),
            }
        } else {
            panic!("expected let");
        }
    }

    #[test]
    fn parse_nested_generic_type() {
        let prog = parse("fn foo() {\n    let x: Option<[int]> = 0\n}");
        let body = &prog.functions[0].node.body.node;
        if let Stmt::Let { ty: Some(ty), .. } = &body.stmts[0].node {
            match &ty.node {
                TypeExpr::Generic { name, type_args } => {
                    assert_eq!(name, "Option");
                    assert_eq!(type_args.len(), 1);
                    assert!(matches!(&type_args[0].node, TypeExpr::Array(_)));
                }
                other => panic!("expected TypeExpr::Generic, got {:?}", other),
            }
        } else {
            panic!("expected let with type annotation");
        }
    }

    #[test]
    fn parse_nested_generic_in_generic() {
        let prog = parse("fn foo() {\n    let x: Pair<int, Option<string>> = 0\n}");
        let body = &prog.functions[0].node.body.node;
        if let Stmt::Let { ty: Some(ty), .. } = &body.stmts[0].node {
            match &ty.node {
                TypeExpr::Generic { name, type_args } => {
                    assert_eq!(name, "Pair");
                    assert_eq!(type_args.len(), 2);
                    assert!(matches!(&type_args[0].node, TypeExpr::Named(n) if n == "int"));
                    assert!(matches!(&type_args[1].node, TypeExpr::Generic { name, .. } if name == "Option"));
                }
                other => panic!("expected TypeExpr::Generic, got {:?}", other),
            }
        } else {
            panic!("expected let with type annotation");
        }
    }

    #[test]
    fn parse_non_generic_function_no_type_params() {
        let prog = parse("fn add(x: int, y: int) int {\n    return x + y\n}");
        assert!(prog.functions[0].node.type_params.is_empty());
    }

    #[test]
    fn parse_non_generic_class_no_type_params() {
        let prog = parse("class Point {\n    x: int\n    y: int\n}");
        assert!(prog.classes[0].node.type_params.is_empty());
    }

    #[test]
    fn parse_class_uses_single() {
        let prog = parse("class Foo uses Logger {\n}");
        let c = &prog.classes[0].node;
        assert_eq!(c.uses.len(), 1);
        assert_eq!(c.uses[0].node, "Logger");
    }

    #[test]
    fn parse_class_uses_multiple_with_bracket_deps() {
        let prog = parse("class Foo uses Logger, Config [db: UserDB] {\n}");
        let c = &prog.classes[0].node;
        assert_eq!(c.uses.len(), 2);
        assert_eq!(c.uses[0].node, "Logger");
        assert_eq!(c.uses[1].node, "Config");
        // bracket dep
        assert_eq!(c.fields.len(), 1);
        assert_eq!(c.fields[0].name.node, "db");
        assert!(c.fields[0].is_injected);
    }

    #[test]
    fn parse_app_ambient() {
        let prog = parse("app MyApp {\n    ambient Logger\n    fn main(self) {\n    }\n}");
        let app = prog.app.as_ref().unwrap();
        assert_eq!(app.node.ambient_types.len(), 1);
        assert_eq!(app.node.ambient_types[0].node, "Logger");
    }

    #[test]
    fn parse_class_uses_missing_comma_rejected() {
        let tokens = lex("class Foo uses Logger Config {\n}").unwrap();
        let mut parser = Parser::new(&tokens, "class Foo uses Logger Config {\n}");
        // Should fail because 'Config' is unexpected after 'Logger' without comma
        // The parser will try to parse 'Config' as bracket deps or impl,
        // and fail because it expects '[', 'impl', or '{' after uses list
        let result = parser.parse_program();
        assert!(result.is_err());
    }

    #[test]
    fn parse_test_block() {
        let src = "test \"hello\" {\n}\n";
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.test_info.len(), 1);
        assert_eq!(prog.test_info[0].display_name, "hello");
        assert_eq!(prog.test_info[0].fn_name, "__test_0");
        assert!(prog.tests.is_none()); // bare test → no tests decl
        assert_eq!(prog.functions.len(), 1);
    }

    #[test]
    fn parse_tests_decl_round_robin() {
        let src = "tests[scheduler: RoundRobin] {\n    test \"rr\" {\n    }\n}\n";
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.test_info.len(), 1);
        assert_eq!(prog.test_info[0].display_name, "rr");
        let tests_decl = prog.tests.as_ref().unwrap();
        assert_eq!(tests_decl.node.strategy, "RoundRobin");
    }

    #[test]
    fn parse_tests_decl_random() {
        let src = "tests[scheduler: Random] {\n    test \"rand\" {\n    }\n}\n";
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.test_info.len(), 1);
        assert_eq!(prog.test_info[0].display_name, "rand");
        let tests_decl = prog.tests.as_ref().unwrap();
        assert_eq!(tests_decl.node.strategy, "Random");
    }

    #[test]
    fn parse_tests_decl_rejects_bare_mix() {
        let src = "test \"bare\" {\n}\ntests[scheduler: RoundRobin] {\n    test \"rr\" {\n    }\n}\n";
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let result = parser.parse_program();
        assert!(result.is_err());
    }

    // Nullable types parser tests

    #[test]
    fn parse_nullable_type_expr() {
        let prog = parse("fn foo(x: int?) int? { return x }");
        let f = &prog.functions[0].node;
        assert_eq!(f.name.node, "foo");
        assert_eq!(f.params.len(), 1);
        // Verify param type is Nullable(Int)
        match &f.params[0].ty.node {
            TypeExpr::Nullable(inner) => {
                assert!(matches!(inner.node, TypeExpr::Named(ref name) if name == "int"));
            }
            _ => panic!("expected nullable type for param"),
        }
        // Verify return type is Nullable(Int)
        let ret_type = f.return_type.as_ref().unwrap();
        match &ret_type.node {
            TypeExpr::Nullable(inner) => {
                assert!(matches!(inner.node, TypeExpr::Named(ref name) if name == "int"));
            }
            _ => panic!("expected nullable return type"),
        }
    }

    #[test]
    fn parse_none_literal() {
        let prog = parse("fn main() { let x = none }");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { value, .. } => {
                assert!(matches!(value.node, Expr::NoneLit));
            }
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_nullable_in_class_field() {
        let prog = parse("class Foo { value: int? }");
        let c = &prog.classes[0].node;
        assert_eq!(c.name.node, "Foo");
        assert_eq!(c.fields.len(), 1);
        match &c.fields[0].ty.node {
            TypeExpr::Nullable(inner) => {
                assert!(matches!(inner.node, TypeExpr::Named(ref name) if name == "int"));
            }
            _ => panic!("expected nullable field type"),
        }
    }

    #[test]
    fn parse_nullable_in_array() {
        let prog = parse("fn foo(xs: [int?]) { }");
        let f = &prog.functions[0].node;
        match &f.params[0].ty.node {
            TypeExpr::Array(elem_type) => {
                match &elem_type.node {
                    TypeExpr::Nullable(inner) => {
                        assert!(matches!(inner.node, TypeExpr::Named(ref name) if name == "int"));
                    }
                    _ => panic!("expected nullable element type"),
                }
            }
            _ => panic!("expected array type"),
        }
    }

    #[test]
    fn parse_question_operator() {
        let prog = parse("fn foo(x: int?) int { return x? }");
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Return(Some(expr)) => {
                assert!(matches!(expr.node, Expr::NullPropagate { .. }));
            }
            _ => panic!("expected return with null propagate"),
        }
    }

    #[test]
    fn parse_nested_nullable_rejected() {
        // Parser doesn't reject nested nullable (typeck does),
        // but we can verify the parse structure
        let tokens = lex("fn main() { let x: int?? = none }");
        // This will fail during lexing/parsing because ?? is not valid
        // The second ? will be parsed as a separate token
        assert!(tokens.is_ok()); // Lexing succeeds
        let tokens_vec = tokens.unwrap();
        let mut parser = Parser::new(&tokens_vec, "fn main() { let x: int?? = none }");
        let result = parser.parse_program();
        // Parser will fail trying to parse the second ?
        assert!(result.is_err());
    }

    #[test]
    fn parse_nullable_map_value() {
        let prog = parse("fn foo() Map<string, int?> { return Map<string, int?> {} }");
        let f = &prog.functions[0].node;
        let ret_type = f.return_type.as_ref().unwrap();
        match &ret_type.node {
            TypeExpr::Generic { name, type_args } => {
                assert_eq!(name, "Map");
                assert_eq!(type_args.len(), 2);
                // Second type arg should be Nullable(Int)
                match &type_args[1].node {
                    TypeExpr::Nullable(inner) => {
                        assert!(matches!(inner.node, TypeExpr::Named(ref n) if n == "int"));
                    }
                    _ => panic!("expected nullable value type in map"),
                }
            }
            _ => panic!("expected generic Map type"),
        }
    }

    #[test]
    fn parse_nullable_in_generic() {
        let prog = parse("enum Option<T> { Some { v: T } None }\nfn main() { let x: Option<int?> = Option<int?>.None }");
        // Verify the type annotation on the let statement
        let f = &prog.functions[0].node;
        match &f.body.node.stmts[0].node {
            Stmt::Let { ty, .. } => {
                let type_ann = ty.as_ref().unwrap();
                match &type_ann.node {
                    TypeExpr::Generic { name, type_args } => {
                        assert_eq!(name, "Option");
                        assert_eq!(type_args.len(), 1);
                        // Type arg should be Nullable(Int)
                        match &type_args[0].node {
                            TypeExpr::Nullable(inner) => {
                                assert!(matches!(inner.node, TypeExpr::Named(ref n) if n == "int"));
                            }
                            _ => panic!("expected nullable type arg"),
                        }
                    }
                    _ => panic!("expected generic type annotation"),
                }
            }
            _ => panic!("expected let statement"),
        }
    }
}

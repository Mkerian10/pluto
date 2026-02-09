pub mod ast;

use std::collections::HashSet;

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
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Spanned<Token>], source: &'a str) -> Self {
        Self { tokens, source, pos: 0, restrict_struct_lit: false, enum_names: HashSet::new() }
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
        if let Some(tok) = self.peek_raw() {
            if matches!(tok.node, Token::Newline) {
                self.advance();
            }
        }
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
        let mut errors = Vec::new();
        self.skip_newlines();

        // Parse imports first
        while self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Import) {
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

            let tok = self.peek().ok_or_else(|| {
                CompileError::syntax("expected declaration after 'pub'", self.eof_span())
            })?;

            match &tok.node {
                Token::App => {
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
                    classes.push(class);
                }
                Token::Fn => {
                    let mut func = self.parse_function()?;
                    func.node.is_pub = is_pub;
                    functions.push(func);
                }
                Token::Trait => {
                    let mut tr = self.parse_trait()?;
                    tr.node.is_pub = is_pub;
                    traits.push(tr);
                }
                Token::Enum => {
                    let mut e = self.parse_enum_decl()?;
                    e.node.is_pub = is_pub;
                    enums.push(e);
                }
                Token::Error => {
                    let mut err_decl = self.parse_error_decl()?;
                    err_decl.node.is_pub = is_pub;
                    errors.push(err_decl);
                }
                Token::Extern => {
                    extern_fns.push(self.parse_extern_fn(is_pub)?);
                }
                _ => {
                    return Err(CompileError::syntax(
                        format!("expected 'fn', 'class', 'trait', 'enum', 'error', 'app', or 'extern', found {}", tok.node),
                        tok.span,
                    ));
                }
            }
            self.skip_newlines();
        }

        Ok(Program { imports, functions, extern_fns, classes, traits, enums, app, errors })
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

        let mut params = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
            if !params.is_empty() {
                self.expect(&Token::Comma)?;
            }
            let pname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let pty = self.parse_type()?;
            params.push(Param { name: pname, ty: pty });
        }
        let close_paren = self.expect(&Token::RParen)?;
        let mut end = close_paren.span.end;

        // Optional return type — if next raw token is not newline/EOF, parse return type
        let return_type = if !self.at_statement_boundary()
            && self.peek().is_some()
            && !matches!(self.peek().unwrap().node, Token::LBrace)
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
        let mut deps = Vec::new();
        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBracket) {
            self.advance(); // consume '['
            while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBracket) {
                if !deps.is_empty() {
                    self.expect(&Token::Comma)?;
                }
                let name = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let ty = self.parse_type()?;
                deps.push(Field { name, ty, is_injected: true });
            }
            self.expect(&Token::RBracket)?;
        }
        Ok(deps)
    }

    fn parse_app_decl(&mut self) -> Result<Spanned<AppDecl>, CompileError> {
        let app_tok = self.expect(&Token::App)?;
        let start = app_tok.span.start;
        let name = self.expect_ident()?;

        let inject_fields = self.parse_bracket_deps()?;

        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            methods.push(self.parse_method()?);
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(AppDecl { name, inject_fields, methods }, Span::new(start, end)))
    }

    fn parse_enum_decl(&mut self) -> Result<Spanned<EnumDecl>, CompileError> {
        let enum_tok = self.expect(&Token::Enum)?;
        let start = enum_tok.span.start;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut variants = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            let vname = self.expect_ident()?;
            let fields = if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBrace) {
                self.expect(&Token::LBrace)?;
                self.skip_newlines();
                let mut fields = Vec::new();
                while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                    if !fields.is_empty() {
                        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                            self.advance();
                        }
                        self.skip_newlines();
                        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
                            break;
                        }
                    }
                    let fname = self.expect_ident()?;
                    self.expect(&Token::Colon)?;
                    let fty = self.parse_type()?;
                    fields.push(Field { name: fname, ty: fty, is_injected: false });
                    self.skip_newlines();
                }
                self.expect(&Token::RBrace)?;
                fields
            } else {
                Vec::new()
            };
            variants.push(EnumVariant { name: vname, fields });
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(EnumDecl { name, type_params, variants, is_pub: false }, Span::new(start, end)))
    }

    fn parse_error_decl(&mut self) -> Result<Spanned<ErrorDecl>, CompileError> {
        let err_tok = self.expect(&Token::Error)?;
        let start = err_tok.span.start;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            if !fields.is_empty() {
                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                    self.advance();
                }
                self.skip_newlines();
                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
                    break;
                }
            }
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let fty = self.parse_type()?;
            fields.push(Field { name: fname, ty: fty, is_injected: false });
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(ErrorDecl { name, fields, is_pub: false }, Span::new(start, end)))
    }

    fn parse_trait(&mut self) -> Result<Spanned<TraitDecl>, CompileError> {
        let trait_tok = self.expect(&Token::Trait)?;
        let start = trait_tok.span.start;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            methods.push(self.parse_trait_method()?);
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(TraitDecl { name, methods, is_pub: false }, Span::new(start, end)))
    }

    fn parse_trait_method(&mut self) -> Result<TraitMethod, CompileError> {
        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        let mut first = true;
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
            if !params.is_empty() || !first {
                self.expect(&Token::Comma)?;
            }
            first = false;

            if params.is_empty() && self.peek().is_some() && matches!(self.peek().unwrap().node, Token::SelfVal) {
                let self_tok = self.advance().unwrap();
                params.push(Param {
                    name: Spanned::new("self".to_string(), self_tok.span),
                    ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                });
            } else {
                let pname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let pty = self.parse_type()?;
                params.push(Param { name: pname, ty: pty });
            }
        }
        self.expect(&Token::RParen)?;

        let return_type = if self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::LBrace | Token::Newline | Token::RBrace) {
            // Check if next non-newline token is '{' — if so, no return type
            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBrace) {
                None
            } else {
                Some(self.parse_type()?)
            }
        } else {
            None
        };

        // If next token is '{', parse a body (default implementation)
        let body = if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBrace) {
            Some(self.parse_block()?)
        } else {
            self.consume_statement_end();
            None
        };

        Ok(TraitMethod { name, params, return_type, body })
    }

    fn parse_class(&mut self) -> Result<Spanned<ClassDecl>, CompileError> {
        let class_tok = self.expect(&Token::Class)?;
        let start = class_tok.span.start;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;

        // Parse optional bracket deps: class Foo[dep: Type]
        let inject_fields = self.parse_bracket_deps()?;

        // Check for `impl Trait1, Trait2`
        let impl_traits = if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Impl) {
            self.advance(); // consume 'impl'
            let mut traits = Vec::new();
            traits.push(self.expect_ident()?);
            while self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
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

        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            if matches!(self.peek().unwrap().node, Token::Fn) {
                methods.push(self.parse_method()?);
            } else {
                let fname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let fty = self.parse_type()?;
                fields.push(Field { name: fname, ty: fty, is_injected: false });
                self.consume_statement_end();
            }
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(ClassDecl { name, type_params, fields, methods, impl_traits, is_pub: false }, Span::new(start, end)))
    }

    fn parse_method(&mut self) -> Result<Spanned<Function>, CompileError> {
        let fn_tok = self.expect(&Token::Fn)?;
        let start = fn_tok.span.start;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        let mut first = true;
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
            if !params.is_empty() || !first {
                self.expect(&Token::Comma)?;
            }
            first = false;

            // Check for `self` as first param
            if params.is_empty() && self.peek().is_some() && matches!(self.peek().unwrap().node, Token::SelfVal) {
                let self_tok = self.advance().unwrap();
                params.push(Param {
                    name: Spanned::new("self".to_string(), self_tok.span),
                    ty: Spanned::new(TypeExpr::Named("Self".to_string()), self_tok.span),
                });
            } else {
                let pname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let pty = self.parse_type()?;
                params.push(Param { name: pname, ty: pty });
            }
        }
        self.expect(&Token::RParen)?;

        let return_type = if self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::LBrace) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Function { name, type_params: vec![], params, return_type, body, is_pub: false },
            Span::new(start, end),
        ))
    }

    /// Parse optional type parameters: `<T>`, `<A, B>`, or empty.
    fn parse_type_params(&mut self) -> Result<Vec<Spanned<String>>, CompileError> {
        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Lt) {
            self.advance(); // consume '<'
            let mut params = Vec::new();
            while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::Gt) {
                if !params.is_empty() {
                    self.expect(&Token::Comma)?;
                }
                params.push(self.expect_ident()?);
            }
            self.expect(&Token::Gt)?;
            Ok(params)
        } else {
            Ok(Vec::new())
        }
    }

    /// Parse a type argument list: `<int, string>`, etc. Assumes we're positioned at `<`.
    fn parse_type_arg_list(&mut self) -> Result<Vec<Spanned<TypeExpr>>, CompileError> {
        self.expect(&Token::Lt)?;
        let mut args = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::Gt) {
            if !args.is_empty() {
                self.expect(&Token::Comma)?;
            }
            args.push(self.parse_type()?);
        }
        self.expect(&Token::Gt)?;
        Ok(args)
    }

    fn parse_function(&mut self) -> Result<Spanned<Function>, CompileError> {
        let fn_tok = self.expect(&Token::Fn)?;
        let start = fn_tok.span.start;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
            if !params.is_empty() {
                self.expect(&Token::Comma)?;
            }
            let pname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let pty = self.parse_type()?;
            params.push(Param { name: pname, ty: pty });
        }
        self.expect(&Token::RParen)?;

        // Return type: if next non-newline token is not '{', it's a return type
        let return_type = if self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::LBrace) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Function { name, type_params, params, return_type, body, is_pub: false },
            Span::new(start, end),
        ))
    }

    fn parse_type(&mut self) -> Result<Spanned<TypeExpr>, CompileError> {
        self.skip_newlines();
        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBracket) {
            let open = self.advance().unwrap();
            let start = open.span.start;
            let inner = self.parse_type()?;
            let close = self.expect(&Token::RBracket)?;
            let end = close.span.end;
            Ok(Spanned::new(TypeExpr::Array(Box::new(inner)), Span::new(start, end)))
        } else if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Fn) {
            // Function type: fn(int, float) string
            let fn_tok = self.advance().unwrap();
            let start = fn_tok.span.start;
            self.expect(&Token::LParen)?;
            let mut params = Vec::new();
            while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
                if !params.is_empty() {
                    self.expect(&Token::Comma)?;
                }
                let ty = self.parse_type()?;
                params.push(Box::new(ty));
            }
            let close_paren = self.expect(&Token::RParen)?;
            let mut end = close_paren.span.end;
            // Optional return type — if next token looks like a type, parse it; otherwise void
            let return_type = if !self.at_statement_boundary()
                && self.peek().is_some()
                && !matches!(self.peek().unwrap().node, Token::LBrace | Token::Comma | Token::RParen | Token::FatArrow | Token::RBracket | Token::Eq)
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
                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Lt) {
                    let type_args = self.parse_type_arg_list()?;
                    let end = type_args.last().map_or(end_span, |_| self.tokens[self.pos - 1].span.end);
                    Ok(Spanned::new(TypeExpr::Generic { name: qualified_name, type_args }, Span::new(ident.span.start, end)))
                } else {
                    let span = Span::new(ident.span.start, end_span);
                    Ok(Spanned::new(TypeExpr::Qualified { module: ident.node, name: type_name.node }, span))
                }
            } else if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Lt) {
                // Generic type: Type<int, string>
                let start = ident.span.start;
                let type_args = self.parse_type_arg_list()?;
                let end = self.tokens[self.pos - 1].span.end; // end of '>'
                Ok(Spanned::new(TypeExpr::Generic { name: ident.node, type_args }, Span::new(start, end)))
            } else {
                Ok(Spanned::new(TypeExpr::Named(ident.node), ident.span))
            }
        }
    }

    fn parse_block(&mut self) -> Result<Spanned<Block>, CompileError> {
        let open = self.expect(&Token::LBrace)?;
        let start = open.span.start;
        let mut stmts = Vec::new();

        self.skip_newlines();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
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
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::For => self.parse_for_stmt(),
            Token::Match => self.parse_match_stmt(),
            Token::Raise => self.parse_raise_stmt(),
            _ => {
                // Parse a full expression, then check for `=` to determine
                // if this is an assignment, field assignment, or expression statement.
                let start = tok.span.start;
                let expr = self.parse_expr(0)?;

                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Eq) {
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

    fn parse_let_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let let_tok = self.expect(&Token::Let)?;
        let start = let_tok.span.start;
        let name = self.expect_ident()?;

        let ty = if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Colon) {
            self.advance(); // consume ':'
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&Token::Eq)?;
        let value = self.parse_expr(0)?;
        let end = value.span.end;
        self.consume_statement_end();

        Ok(Spanned::new(Stmt::Let { name, ty, value }, Span::new(start, end)))
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

    fn parse_if_stmt(&mut self) -> Result<Spanned<Stmt>, CompileError> {
        let if_tok = self.expect(&Token::If)?;
        let start = if_tok.span.start;
        let old_restrict = self.restrict_struct_lit;
        self.restrict_struct_lit = true;
        let condition = self.parse_expr(0)?;
        self.restrict_struct_lit = old_restrict;
        let then_block = self.parse_block()?;

        let else_block = if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Else) {
            self.advance(); // consume 'else'
            Some(self.parse_block()?)
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
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            let first_name = self.expect_ident()?;
            self.expect(&Token::Dot)?;
            let second_name = self.expect_ident()?;

            // Check if this is module.Enum.Variant (qualified) or Enum.Variant (local)
            let (enum_name, variant_name) = if self.peek().is_some()
                && matches!(self.peek().unwrap().node, Token::Dot)
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
                while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                    if !bindings.is_empty() {
                        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                            self.advance();
                        }
                        self.skip_newlines();
                        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
                            break;
                        }
                    }
                    let field_name = self.expect_ident()?;
                    let rename = if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Colon) {
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

            arms.push(MatchArm { enum_name, variant_name, type_args: vec![], bindings, body });
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
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            if !fields.is_empty() {
                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                    self.advance();
                }
                self.skip_newlines();
                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
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
            Stmt::Raise { error_name, fields },
            Span::new(start, end),
        ))
    }

    fn parse_catch_handler(&mut self) -> Result<(CatchHandler, usize), CompileError> {
        // Lookahead: if ident followed by {, it's wildcard form
        if self.is_catch_wildcard_ahead() {
            let var = self.expect_ident()?;
            self.expect(&Token::LBrace)?;
            self.skip_newlines();
            let body = self.parse_expr(0)?;
            self.skip_newlines();
            let close = self.expect(&Token::RBrace)?;
            Ok((CatchHandler::Wildcard { var, body: Box::new(body) }, close.span.end))
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

        loop {
            let Some(tok) = self.peek() else { break };

            // Dot notation (postfix) — highest precedence
            if matches!(tok.node, Token::Dot) {
                self.skip_newlines();
                self.advance(); // consume '.'
                let field_name = self.expect_ident()?;

                // Check if it's a method call
                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
                        if !args.is_empty() {
                            self.expect(&Token::Comma)?;
                        }
                        args.push(self.parse_expr(0)?);
                    }
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
                        && matches!(self.peek().unwrap().node, Token::LBrace)
                        && self.is_struct_lit_ahead()
                    {
                        // EnumName.Variant { field: value }
                        self.advance(); // consume '{'
                        self.skip_newlines();
                        let mut fields = Vec::new();
                        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                            if !fields.is_empty() {
                                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                                    self.advance();
                                }
                                self.skip_newlines();
                                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
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
                        let span = Span::new(lhs.span.start, close.span.end);
                        lhs = Spanned::new(
                            Expr::EnumData {
                                enum_name: Spanned::new(enum_name_str, enum_name_span),
                                variant: field_name,
                                type_args: vec![],
                                fields,
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
                            },
                            span,
                        );
                    }
                } else if !self.restrict_struct_lit
                    && matches!(&lhs.node, Expr::Ident(_))
                    && self.peek().is_some()
                    && matches!(self.peek().unwrap().node, Token::LBrace)
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
                    self.skip_newlines();
                    let mut fields = Vec::new();
                    while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                        if !fields.is_empty() {
                            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                                self.advance();
                            }
                            self.skip_newlines();
                            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
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
                    let span = Span::new(lhs.span.start, close.span.end);
                    lhs = Spanned::new(
                        Expr::StructLit {
                            name: Spanned::new(qualified_name, name_span),
                            type_args: vec![],
                            fields,
                        },
                        span,
                    );
                } else if matches!(&lhs.node, Expr::FieldAccess { object, .. } if matches!(&object.node, Expr::Ident(_))) {
                    // Possible qualified enum: module.Enum.Variant or module.Enum.Variant { fields }
                    let (module_name, enum_local) = match &lhs.node {
                        Expr::FieldAccess { object, field } => {
                            match &object.node {
                                Expr::Ident(n) => (n.clone(), field.node.clone()),
                                _ => unreachable!(),
                            }
                        }
                        _ => unreachable!(),
                    };
                    let qualified_enum_name = format!("{}.{}", module_name, enum_local);
                    let enum_name_span = Span::new(lhs.span.start, field_name.span.end);

                    if !self.restrict_struct_lit
                        && self.peek().is_some()
                        && matches!(self.peek().unwrap().node, Token::LBrace)
                        && self.is_struct_lit_ahead()
                    {
                        // module.Enum.Variant { field: value }
                        self.advance(); // consume '{'
                        self.skip_newlines();
                        let mut fields = Vec::new();
                        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                            if !fields.is_empty() {
                                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                                    self.advance();
                                }
                                self.skip_newlines();
                                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
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
                        let span = Span::new(lhs.span.start, close.span.end);
                        lhs = Spanned::new(
                            Expr::EnumData {
                                enum_name: Spanned::new(qualified_enum_name, enum_name_span),
                                variant: field_name,
                                type_args: vec![],
                                fields,
                            },
                            span,
                        );
                    } else {
                        // module.Enum.Variant (unit) — speculatively treat as enum
                        // Typeck will reject if it's not actually an enum
                        let span = Span::new(lhs.span.start, field_name.span.end);
                        lhs = Spanned::new(
                            Expr::EnumUnit {
                                enum_name: Spanned::new(qualified_enum_name, enum_name_span),
                                variant: field_name,
                                type_args: vec![],
                            },
                            span,
                        );
                    }
                } else {
                    let span = Span::new(lhs.span.start, field_name.span.end);
                    lhs = Spanned::new(
                        Expr::FieldAccess {
                            object: Box::new(lhs),
                            field: field_name,
                        },
                        span,
                    );
                }
                continue;
            }

            // Index operator: arr[i] — use peek_raw to prevent newline-separated
            // expressions from being parsed as indexing (e.g. arr\n[1,2])
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::LBracket) {
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

            // Postfix ? — reserved for future Option/null handling
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Question) {
                return Err(CompileError::syntax(
                    "? is reserved for future Option/null handling; use ! for error propagation",
                    self.peek_raw().unwrap().span,
                ));
            }

            // Postfix ! — error propagation (must be on same line via peek_raw)
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Bang) {
                let bang = self.advance().unwrap();
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
                    && matches!(self.peek().unwrap().node, Token::LBrace)
                    && self.is_struct_lit_ahead()
                {
                    // EnumName<type_args>.Variant { field: value }
                    self.advance(); // consume '{'
                    self.skip_newlines();
                    let mut fields = Vec::new();
                    while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                        if !fields.is_empty() {
                            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                                self.advance();
                            }
                            self.skip_newlines();
                            if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
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
                    let span = Span::new(lhs.span.start, close.span.end);
                    lhs = Spanned::new(
                        Expr::EnumData {
                            enum_name: Spanned::new(enum_name_str, enum_name_span),
                            variant,
                            type_args,
                            fields,
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
                        },
                        span,
                    );
                }
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
                let tok = self.advance().unwrap();
                let Token::IntLit(n) = &tok.node else { unreachable!() };
                Ok(Spanned::new(Expr::IntLit(*n), tok.span))
            }
            Token::FloatLit(_) => {
                let tok = self.advance().unwrap();
                let Token::FloatLit(n) = &tok.node else { unreachable!() };
                Ok(Spanned::new(Expr::FloatLit(*n), tok.span))
            }
            Token::True => {
                let tok = self.advance().unwrap();
                Ok(Spanned::new(Expr::BoolLit(true), tok.span))
            }
            Token::False => {
                let tok = self.advance().unwrap();
                Ok(Spanned::new(Expr::BoolLit(false), tok.span))
            }
            Token::StringLit(_) => {
                let tok = self.advance().unwrap();
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
                let tok = self.advance().unwrap();
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
                let tok = self.advance().unwrap();
                let start = tok.span.start;
                let operand = self.parse_prefix()?;
                let end = operand.span.end;
                Ok(Spanned::new(
                    Expr::UnaryOp { op: UnaryOp::Neg, operand: Box::new(operand) },
                    Span::new(start, end),
                ))
            }
            Token::Bang => {
                let tok = self.advance().unwrap();
                let start = tok.span.start;
                let operand = self.parse_prefix()?;
                let end = operand.span.end;
                Ok(Spanned::new(
                    Expr::UnaryOp { op: UnaryOp::Not, operand: Box::new(operand) },
                    Span::new(start, end),
                ))
            }
            Token::LBracket => {
                let tok = self.advance().unwrap();
                let start = tok.span.start;
                let mut elements = Vec::new();
                while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBracket) {
                    if !elements.is_empty() {
                        self.expect(&Token::Comma)?;
                    }
                    elements.push(self.parse_expr(0)?);
                }
                let close = self.expect(&Token::RBracket)?;
                let end = close.span.end;
                if elements.is_empty() {
                    return Err(CompileError::syntax(
                        "empty array literals are not supported",
                        Span::new(start, end),
                    ));
                }
                Ok(Spanned::new(Expr::ArrayLit { elements }, Span::new(start, end)))
            }
            Token::Question => {
                return Err(CompileError::syntax(
                    "? is reserved for future Option/null handling; use ! for error propagation",
                    tok.span,
                ));
            }
            _ => Err(CompileError::syntax(
                format!("unexpected token {} in expression", tok.node),
                tok.span,
            )),
        }
    }

    /// Continue parsing an expression that started with an identifier (handles calls, struct literals, and plain ident).
    fn parse_expr_after_ident(&mut self, ident: Spanned<String>) -> Result<Spanned<Expr>, CompileError> {
        // Check for function call
        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LParen) {
            self.advance(); // consume '('
            let mut args = Vec::new();
            while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
                if !args.is_empty() {
                    self.expect(&Token::Comma)?;
                }
                args.push(self.parse_expr(0)?);
            }
            let close = self.expect(&Token::RParen)?;
            let span = Span::new(ident.span.start, close.span.end);
            Ok(Spanned::new(Expr::Call { name: ident, args }, span))
        } else if !self.restrict_struct_lit && self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBrace) {
            // Check if this looks like a struct literal: Ident { field: value, ... }
            // We need to distinguish from a block. A struct literal has `ident : expr` inside.
            // Use a lookahead: after `{`, if we see `ident :` it's a struct literal.
            if self.is_struct_lit_ahead() {
                self.advance(); // consume '{'
                self.skip_newlines();
                let mut fields = Vec::new();
                while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                    if !fields.is_empty() {
                        // Allow comma or newline as separator
                        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                            self.advance();
                        }
                        self.skip_newlines();
                        if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
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
                let span = Span::new(ident.span.start, close.span.end);
                Ok(Spanned::new(Expr::StructLit { name: ident, type_args: vec![], fields }, span))
            } else {
                Ok(Spanned::new(Expr::Ident(ident.node.clone()), ident.span))
            }
        } else if !self.restrict_struct_lit
            && self.peek().is_some()
            && matches!(self.peek().unwrap().node, Token::Lt)
            && self.is_generic_struct_lit_ahead()
        {
            // Generic struct literal: Ident<type_args> { field: value, ... }
            let start = ident.span.start;
            let type_args = self.parse_type_arg_list()?;
            self.advance(); // consume '{'
            self.skip_newlines();
            let mut fields = Vec::new();
            while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
                if !fields.is_empty() {
                    if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Comma) {
                        self.advance();
                    }
                    self.skip_newlines();
                    if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::RBrace) {
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
            let span = Span::new(start, close.span.end);
            Ok(Spanned::new(Expr::StructLit { name: ident, type_args, fields }, span))
        } else {
            Ok(Spanned::new(Expr::Ident(ident.node.clone()), ident.span))
        }
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
        if parts.len() == 1 {
            if let StringInterpPart::Lit(s) = &parts[0] {
                return Ok(Spanned::new(Expr::StringLit(s.clone()), span));
            }
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
        let mut params = Vec::new();
        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RParen) {
            if !params.is_empty() {
                self.expect(&Token::Comma)?;
            }
            let pname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let pty = self.parse_type()?;
            params.push(Param { name: pname, ty: pty });
        }
        self.expect(&Token::RParen)?;

        // Optional return type: if next non-newline token is NOT `=>`, parse a type first
        let return_type = if self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::FatArrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&Token::FatArrow)?;

        // Body: either a block `{ ... }` or a single expression (desugared to return stmt)
        let body = if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::LBrace) {
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
        // We're positioned at `{`. Look past it for `ident :`
        let mut i = self.pos + 1;
        // skip newlines
        while i < self.tokens.len() && matches!(self.tokens[i].node, Token::Newline) {
            i += 1;
        }
        if i >= self.tokens.len() {
            return false;
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
}

fn infix_binding_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Or => (1, 2),
        BinOp::And => (3, 4),
        BinOp::Eq | BinOp::Neq => (5, 6),
        BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => (7, 8),
        BinOp::Add | BinOp::Sub => (9, 10),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (11, 12),
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
                Expr::StructLit { name, type_args, fields } => {
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
                Expr::EnumUnit { enum_name, variant, type_args } => {
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
                Expr::EnumData { enum_name, variant, fields, type_args } => {
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
}

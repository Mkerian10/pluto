pub mod ast;

use crate::diagnostics::CompileError;
use crate::lexer::token::Token;
use crate::span::{Span, Spanned};
use ast::*;

pub struct Parser<'a> {
    tokens: &'a [Spanned<Token>],
    source: &'a str,
    pos: usize,
    restrict_struct_lit: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Spanned<Token>], source: &'a str) -> Self {
        Self { tokens, source, pos: 0, restrict_struct_lit: false }
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

    pub fn parse_program(&mut self) -> Result<Program, CompileError> {
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut classes = Vec::new();
        let mut traits = Vec::new();
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
                _ => {
                    return Err(CompileError::syntax(
                        format!("expected 'fn', 'class', or 'trait', found {}", tok.node),
                        tok.span,
                    ));
                }
            }
            self.skip_newlines();
        }

        Ok(Program { imports, functions, classes, traits })
    }

    fn parse_import(&mut self) -> Result<Spanned<ImportDecl>, CompileError> {
        let import_tok = self.expect(&Token::Import)?;
        let start = import_tok.span.start;
        let module_name = self.expect_ident()?;
        let end = module_name.span.end;
        self.consume_statement_end();
        Ok(Spanned::new(ImportDecl { module_name }, Span::new(start, end)))
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

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.peek().is_some() && !matches!(self.peek().unwrap().node, Token::RBrace) {
            if matches!(self.peek().unwrap().node, Token::Fn) {
                methods.push(self.parse_method()?);
            } else {
                let fname = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let fty = self.parse_type()?;
                fields.push(Field { name: fname, ty: fty });
                self.consume_statement_end();
            }
            self.skip_newlines();
        }

        let close = self.expect(&Token::RBrace)?;
        let end = close.span.end;

        Ok(Spanned::new(ClassDecl { name, fields, methods, impl_traits, is_pub: false }, Span::new(start, end)))
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
            Function { name, params, return_type, body, is_pub: false },
            Span::new(start, end),
        ))
    }

    fn parse_function(&mut self) -> Result<Spanned<Function>, CompileError> {
        let fn_tok = self.expect(&Token::Fn)?;
        let start = fn_tok.span.start;
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
            Function { name, params, return_type, body, is_pub: false },
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
        } else {
            let ident = self.expect_ident()?;
            // Check for qualified type: module.Type
            if self.peek_raw().is_some() && matches!(self.peek_raw().unwrap().node, Token::Dot) {
                self.advance(); // consume '.'
                let type_name = self.expect_ident()?;
                let span = Span::new(ident.span.start, type_name.span.end);
                Ok(Spanned::new(TypeExpr::Qualified { module: ident.node, name: type_name.node }, span))
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
                            fields,
                        },
                        span,
                    );
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
                Ok(Spanned::new(Expr::StringLit(s.clone()), tok.span))
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
                self.advance(); // consume '('
                let old_restrict = self.restrict_struct_lit;
                self.restrict_struct_lit = false;
                let expr = self.parse_expr(0)?;
                self.restrict_struct_lit = old_restrict;
                self.expect(&Token::RParen)?;
                Ok(expr)
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
                Ok(Spanned::new(Expr::StructLit { name: ident, fields }, span))
            } else {
                Ok(Spanned::new(Expr::Ident(ident.node.clone()), ident.span))
            }
        } else {
            Ok(Spanned::new(Expr::Ident(ident.node.clone()), ident.span))
        }
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
                    Expr::StructLit { name, fields } => {
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
        assert_eq!(prog.imports[0].node.module_name.node, "math");
        assert_eq!(prog.functions.len(), 1);
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
                    Expr::StructLit { name, fields } => {
                        assert_eq!(name.node, "math.Point");
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("expected struct literal, got {:?}", value.node),
                }
            }
            _ => panic!("expected let"),
        }
    }
}

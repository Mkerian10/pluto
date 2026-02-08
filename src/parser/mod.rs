pub mod ast;

use crate::diagnostics::CompileError;
use crate::lexer::token::Token;
use crate::span::{Span, Spanned};
use ast::*;

pub struct Parser<'a> {
    tokens: &'a [Spanned<Token>],
    source: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Spanned<Token>], source: &'a str) -> Self {
        Self { tokens, source, pos: 0 }
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
        let mut functions = Vec::new();
        self.skip_newlines();

        while self.peek().is_some() {
            functions.push(self.parse_function()?);
            self.skip_newlines();
        }

        Ok(Program { functions })
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
            Function { name, params, return_type, body },
            Span::new(start, end),
        ))
    }

    fn parse_type(&mut self) -> Result<Spanned<TypeExpr>, CompileError> {
        let ident = self.expect_ident()?;
        Ok(Spanned::new(TypeExpr::Named(ident.node), ident.span))
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
            Token::Ident => {
                // Could be assignment or expression statement
                // Look ahead for '='
                let start = tok.span.start;
                let ident = self.expect_ident()?;

                if self.peek().is_some() && matches!(self.peek().unwrap().node, Token::Eq) {
                    self.advance(); // consume '='
                    let value = self.parse_expr(0)?;
                    let end = value.span.end;
                    self.consume_statement_end();
                    Ok(Spanned::new(
                        Stmt::Assign { target: ident, value },
                        Span::new(start, end),
                    ))
                } else {
                    // It's an expression statement starting with an identifier
                    // We need to continue parsing the expression
                    let expr = self.parse_expr_after_ident(ident)?;
                    let end = expr.span.end;
                    self.consume_statement_end();
                    Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
                }
            }
            _ => {
                let start = tok.span.start;
                let expr = self.parse_expr(0)?;
                let end = expr.span.end;
                self.consume_statement_end();
                Ok(Spanned::new(Stmt::Expr(expr), Span::new(start, end)))
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
        let condition = self.parse_expr(0)?;
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
        let condition = self.parse_expr(0)?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Spanned::new(
            Stmt::While { condition, body },
            Span::new(start, end),
        ))
    }

    // Pratt parser for expressions
    fn parse_expr(&mut self, min_bp: u8) -> Result<Spanned<Expr>, CompileError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            let Some(tok) = self.peek() else { break };
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
            Token::LParen => {
                self.advance(); // consume '('
                let expr = self.parse_expr(0)?;
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
            _ => Err(CompileError::syntax(
                format!("unexpected token {} in expression", tok.node),
                tok.span,
            )),
        }
    }

    /// Continue parsing an expression that started with an identifier (handles calls and infix).
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
        } else {
            Ok(Spanned::new(Expr::Ident(ident.node.clone()), ident.span))
        }
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
}

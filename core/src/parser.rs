use std::collections::HashSet;

use crate::ast::{BinaryOp, Decl, DeclKind, Entry, Expr, ExprKind, File, ObjectField, UnaryOp};
use crate::types::{ErrorCode, ErrorLevel, LangError, Position, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Eof,
    Ident,
    Number,
    String,
    Solid,
    Feature,
    Let,
    True,
    False,
    Null,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Semicolon,
    Comma,
    Dot,
    Assign,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    EqEq,
    NotEq,
    Lt,
    Lte,
    Gt,
    Gte,
    AndAnd,
    OrOr,
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    lexeme: String,
    span: Span,
}

struct Lexer<'a> {
    src: &'a str,
    idx: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            idx: 0,
            line: 1,
            col: 1,
        }
    }

    fn tokenize(mut self) -> Result<Vec<Token>, LangError> {
        let mut out = Vec::new();
        loop {
            self.skip_ws_and_comments()?;
            let start = self.position();
            let Some(ch) = self.peek_char() else {
                out.push(Token {
                    kind: TokenKind::Eof,
                    lexeme: String::new(),
                    span: Span { start, end: start },
                });
                break;
            };

            let token = match ch {
                '{' => self.single(TokenKind::LBrace),
                '}' => self.single(TokenKind::RBrace),
                '(' => self.single(TokenKind::LParen),
                ')' => self.single(TokenKind::RParen),
                '[' => self.single(TokenKind::LBracket),
                ']' => self.single(TokenKind::RBracket),
                ':' => self.single(TokenKind::Colon),
                ';' => self.single(TokenKind::Semicolon),
                ',' => self.single(TokenKind::Comma),
                '.' => self.single(TokenKind::Dot),
                '+' => self.single(TokenKind::Plus),
                '-' => self.single(TokenKind::Minus),
                '*' => self.single(TokenKind::Star),
                '%' => self.single(TokenKind::Percent),
                '=' => {
                    self.bump_char();
                    if self.consume_char('=') {
                        self.mk_token(TokenKind::EqEq, start)
                    } else {
                        self.mk_token(TokenKind::Assign, start)
                    }
                }
                '!' => {
                    self.bump_char();
                    if self.consume_char('=') {
                        self.mk_token(TokenKind::NotEq, start)
                    } else {
                        self.mk_token(TokenKind::Bang, start)
                    }
                }
                '<' => {
                    self.bump_char();
                    if self.consume_char('=') {
                        self.mk_token(TokenKind::Lte, start)
                    } else {
                        self.mk_token(TokenKind::Lt, start)
                    }
                }
                '>' => {
                    self.bump_char();
                    if self.consume_char('=') {
                        self.mk_token(TokenKind::Gte, start)
                    } else {
                        self.mk_token(TokenKind::Gt, start)
                    }
                }
                '&' => {
                    self.bump_char();
                    if self.consume_char('&') {
                        self.mk_token(TokenKind::AndAnd, start)
                    } else {
                        return Err(LangError::syntax(
                            Span {
                                start,
                                end: self.position(),
                            },
                            "unexpected '&', expected '&&'",
                        ));
                    }
                }
                '|' => {
                    self.bump_char();
                    if self.consume_char('|') {
                        self.mk_token(TokenKind::OrOr, start)
                    } else {
                        return Err(LangError::syntax(
                            Span {
                                start,
                                end: self.position(),
                            },
                            "unexpected '|', expected '||'",
                        ));
                    }
                }
                '/' => self.single(TokenKind::Slash),
                '"' => self.read_string()?,
                c if c.is_ascii_digit() => self.read_number(),
                c if is_ident_start(c) => self.read_ident_or_keyword(),
                _ => {
                    return Err(LangError::syntax(
                        Span {
                            start,
                            end: self.position_after_one_char(),
                        },
                        format!("unexpected character '{ch}'"),
                    ));
                }
            };

            out.push(token);
        }

        Ok(out)
    }

    fn position(&self) -> Position {
        Position {
            line: self.line,
            col: self.col,
            offset: self.idx,
        }
    }

    fn position_after_one_char(&self) -> Position {
        if let Some(ch) = self.peek_char() {
            Position {
                line: self.line,
                col: self.col + ch.len_utf8(),
                offset: self.idx + ch.len_utf8(),
            }
        } else {
            self.position()
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.idx..].chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.idx += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.bump_char();
            true
        } else {
            false
        }
    }

    fn mk_token(&self, kind: TokenKind, start: Position) -> Token {
        let end = self.position();
        let lexeme = self.src[start.offset..end.offset].to_owned();
        Token {
            kind,
            lexeme,
            span: Span { start, end },
        }
    }

    fn single(&mut self, kind: TokenKind) -> Token {
        let start = self.position();
        self.bump_char();
        self.mk_token(kind, start)
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), LangError> {
        loop {
            while let Some(ch) = self.peek_char() {
                if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                    self.bump_char();
                } else {
                    break;
                }
            }

            if self.peek_char() != Some('/') {
                break;
            }

            let start = self.position();
            let mut chars = self.src[self.idx..].chars();
            let _slash = chars.next();
            match chars.next() {
                Some('/') => {
                    self.bump_char();
                    self.bump_char();
                    while let Some(ch) = self.peek_char() {
                        self.bump_char();
                        if ch == '\n' {
                            break;
                        }
                    }
                }
                Some('*') => {
                    self.bump_char();
                    self.bump_char();
                    let mut closed = false;
                    while let Some(ch) = self.bump_char() {
                        if ch == '*' && self.peek_char() == Some('/') {
                            self.bump_char();
                            closed = true;
                            break;
                        }
                    }
                    if !closed {
                        return Err(LangError::syntax(
                            Span {
                                start,
                                end: self.position(),
                            },
                            "unterminated block comment",
                        ));
                    }
                }
                _ => break,
            }
        }

        Ok(())
    }

    fn read_number(&mut self) -> Token {
        let start = self.position();
        while matches!(self.peek_char(), Some(c) if c.is_ascii_digit()) {
            self.bump_char();
        }

        if self.peek_char() == Some('.') {
            let mut chars = self.src[self.idx..].chars();
            let _dot = chars.next();
            if matches!(chars.next(), Some(c) if c.is_ascii_digit()) {
                self.bump_char();
                while matches!(self.peek_char(), Some(c) if c.is_ascii_digit()) {
                    self.bump_char();
                }
            }
        }

        if matches!(self.peek_char(), Some('e' | 'E')) {
            let save = (self.idx, self.line, self.col);
            self.bump_char();
            if matches!(self.peek_char(), Some('+' | '-')) {
                self.bump_char();
            }
            if matches!(self.peek_char(), Some(c) if c.is_ascii_digit()) {
                while matches!(self.peek_char(), Some(c) if c.is_ascii_digit()) {
                    self.bump_char();
                }
            } else {
                self.idx = save.0;
                self.line = save.1;
                self.col = save.2;
            }
        }

        self.mk_token(TokenKind::Number, start)
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        let start = self.position();
        self.bump_char();
        while matches!(self.peek_char(), Some(c) if is_ident_continue(c)) {
            self.bump_char();
        }
        let token = self.mk_token(TokenKind::Ident, start);
        let kind = match token.lexeme.as_str() {
            "solid" => TokenKind::Solid,
            "feature" => TokenKind::Feature,
            "let" => TokenKind::Let,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            _ => TokenKind::Ident,
        };
        Token { kind, ..token }
    }

    fn read_string(&mut self) -> Result<Token, LangError> {
        let start = self.position();
        self.bump_char();

        while let Some(ch) = self.peek_char() {
            if ch == '"' {
                self.bump_char();
                return Ok(self.mk_token(TokenKind::String, start));
            }

            if ch == '\\' {
                self.bump_char();
                let Some(esc) = self.bump_char() else {
                    return Err(LangError::syntax(
                        Span {
                            start,
                            end: self.position(),
                        },
                        "unterminated string escape",
                    ));
                };

                if !matches!(esc, '"' | '\\' | 'n' | 't' | 'r') {
                    return Err(LangError::syntax(
                        Span {
                            start,
                            end: self.position(),
                        },
                        format!("invalid string escape \\{esc}"),
                    ));
                }
            } else {
                self.bump_char();
            }
        }

        Err(LangError::syntax(
            Span {
                start,
                end: self.position(),
            },
            "unterminated string literal",
        ))
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

struct AstParser {
    tokens: Vec<Token>,
    idx: usize,
}

impl AstParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, idx: 0 }
    }

    fn parse_file(&mut self) -> Result<File, LangError> {
        let start = self.current().span.start;
        let mut decls = Vec::new();
        let mut seen = HashSet::new();
        while !self.at(TokenKind::Eof) {
            let decl = self.parse_decl()?;
            if !seen.insert(decl.id.clone()) {
                return Err(LangError {
                    level: ErrorLevel::Error,
                    code: ErrorCode::DuplicateId,
                    message: format!("duplicate declaration id '{}'", decl.id),
                    span: decl.span,
                    node: Some(decl.id.clone()),
                    details: Vec::new(),
                });
            }
            decls.push(decl);
        }
        let end = self.current().span.end;
        Ok(File {
            decls,
            span: Span { start, end },
        })
    }

    fn parse_decl(&mut self) -> Result<Decl, LangError> {
        let start = self.current().span.start;
        let kind = if self.matches(TokenKind::Solid) {
            DeclKind::Solid
        } else if self.matches(TokenKind::Feature) {
            DeclKind::Feature
        } else {
            return Err(self.expected("'solid' or 'feature'"));
        };

        let id = self.expect_ident("declaration id")?;
        self.expect(TokenKind::Assign, "'='")?;
        let op = self.expect_ident("op name")?;
        let entries = self.parse_block()?;
        self.matches(TokenKind::Semicolon);
        let end = self.previous().span.end;

        Ok(Decl {
            kind,
            id,
            op,
            entries,
            span: Span { start, end },
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Entry>, LangError> {
        self.expect(TokenKind::LBrace, "'{'")?;
        let mut entries = Vec::new();
        while !self.at(TokenKind::RBrace) {
            if self.at(TokenKind::Eof) {
                return Err(self.expected("'}'"));
            }
            entries.push(self.parse_entry()?);
        }
        self.expect(TokenKind::RBrace, "'}'")?;
        Ok(entries)
    }

    fn parse_entry(&mut self) -> Result<Entry, LangError> {
        if self.matches(TokenKind::Let) {
            let start = self.previous().span.start;
            let name = self.expect_ident("let name")?;
            self.expect(TokenKind::Assign, "'='")?;
            let expr = self.parse_expr()?;
            let semi = self.expect(TokenKind::Semicolon, "';'")?;
            return Ok(Entry::Let {
                name,
                expr,
                span: Span {
                    start,
                    end: semi.span.end,
                },
            });
        }

        let start = self.current().span.start;
        let name = self.expect_ident("field name")?;
        self.expect(TokenKind::Colon, "':'")?;
        let expr = self.parse_expr()?;
        let semi = self.expect(TokenKind::Semicolon, "';'")?;
        Ok(Entry::Field {
            name,
            expr,
            span: Span {
                start,
                end: semi.span.end,
            },
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, LangError> {
        self.parse_logic_or()
    }

    fn parse_logic_or(&mut self) -> Result<Expr, LangError> {
        let mut expr = self.parse_logic_and()?;
        while self.matches(TokenKind::OrOr) {
            let op_span = self.previous().span;
            let right = self.parse_logic_and()?;
            let span = Span::merge(expr.span, right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::Or,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span: Span::merge(span, op_span),
            };
        }
        Ok(expr)
    }

    fn parse_logic_and(&mut self) -> Result<Expr, LangError> {
        let mut expr = self.parse_equality()?;
        while self.matches(TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            let span = Span::merge(expr.span, right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::And,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, LangError> {
        let mut expr = self.parse_compare()?;
        loop {
            let op = if self.matches(TokenKind::EqEq) {
                Some(BinaryOp::Eq)
            } else if self.matches(TokenKind::NotEq) {
                Some(BinaryOp::Ne)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_compare()?;
            let span = Span::merge(expr.span, right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_compare(&mut self) -> Result<Expr, LangError> {
        let mut expr = self.parse_add()?;
        loop {
            let op = if self.matches(TokenKind::Lt) {
                Some(BinaryOp::Lt)
            } else if self.matches(TokenKind::Lte) {
                Some(BinaryOp::Le)
            } else if self.matches(TokenKind::Gt) {
                Some(BinaryOp::Gt)
            } else if self.matches(TokenKind::Gte) {
                Some(BinaryOp::Ge)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_add()?;
            let span = Span::merge(expr.span, right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_add(&mut self) -> Result<Expr, LangError> {
        let mut expr = self.parse_mul()?;
        loop {
            let op = if self.matches(TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.matches(TokenKind::Minus) {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_mul()?;
            let span = Span::merge(expr.span, right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_mul(&mut self) -> Result<Expr, LangError> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches(TokenKind::Star) {
                Some(BinaryOp::Mul)
            } else if self.matches(TokenKind::Slash) {
                Some(BinaryOp::Div)
            } else if self.matches(TokenKind::Percent) {
                Some(BinaryOp::Mod)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_unary()?;
            let span = Span::merge(expr.span, right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, LangError> {
        if self.matches(TokenKind::Minus) {
            let start = self.previous().span.start;
            let expr = self.parse_unary()?;
            let span = Span {
                start,
                end: expr.span.end,
            };
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                },
                span,
            });
        }

        if self.matches(TokenKind::Bang) {
            let start = self.previous().span.start;
            let expr = self.parse_unary()?;
            let span = Span {
                start,
                end: expr.span.end,
            };
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                },
                span,
            });
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, LangError> {
        if self.matches(TokenKind::Number) {
            let tok = self.previous().clone();
            let value = tok.lexeme.parse::<f64>().map_err(|_| {
                LangError::syntax(
                    tok.span,
                    format!("invalid numeric literal '{}'", tok.lexeme),
                )
            })?;
            return Ok(Expr {
                kind: ExprKind::Number(value),
                span: tok.span,
            });
        }

        if self.matches(TokenKind::String) {
            let tok = self.previous().clone();
            let value = unescape_string(&tok.lexeme, tok.span)?;
            return Ok(Expr {
                kind: ExprKind::String(value),
                span: tok.span,
            });
        }

        if self.matches(TokenKind::True) {
            return Ok(Expr {
                kind: ExprKind::Bool(true),
                span: self.previous().span,
            });
        }

        if self.matches(TokenKind::False) {
            return Ok(Expr {
                kind: ExprKind::Bool(false),
                span: self.previous().span,
            });
        }

        if self.matches(TokenKind::Null) {
            return Ok(Expr {
                kind: ExprKind::Null,
                span: self.previous().span,
            });
        }

        if self.matches(TokenKind::LBracket) {
            return self.finish_vector();
        }

        if self.matches(TokenKind::LBrace) {
            return self.finish_object();
        }

        if self.matches(TokenKind::Ident) {
            let first = self.previous().clone();

            if self.matches(TokenKind::LParen) {
                let args = self.parse_args()?;
                let end = self.expect(TokenKind::RParen, "')'")?.span.end;
                return Ok(Expr {
                    kind: ExprKind::Call {
                        name: first.lexeme,
                        args,
                    },
                    span: Span {
                        start: first.span.start,
                        end,
                    },
                });
            }

            if self.matches(TokenKind::Dot) {
                let mut segments = vec![first.lexeme];
                segments.push(self.expect_ident("reference segment")?);
                while self.matches(TokenKind::Dot) {
                    segments.push(self.expect_ident("reference segment")?);
                }
                return Ok(Expr {
                    kind: ExprKind::Reference(segments),
                    span: Span {
                        start: first.span.start,
                        end: self.previous().span.end,
                    },
                });
            }

            return Ok(Expr {
                kind: ExprKind::Ident(first.lexeme),
                span: first.span,
            });
        }

        if self.matches(TokenKind::LParen) {
            let start = self.previous().span.start;
            let expr = self.parse_expr()?;
            let end = self.expect(TokenKind::RParen, "')'")?.span.end;
            return Ok(Expr {
                span: Span { start, end },
                kind: expr.kind,
            });
        }

        Err(self.expected("expression"))
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, LangError> {
        let mut args = Vec::new();
        if self.at(TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if !self.matches(TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    fn finish_vector(&mut self) -> Result<Expr, LangError> {
        let start = self.previous().span.start;
        let x = self.parse_expr()?;
        self.expect(TokenKind::Comma, "','")?;
        let y = self.parse_expr()?;
        self.expect(TokenKind::Comma, "','")?;
        let z = self.parse_expr()?;
        let end = self.expect(TokenKind::RBracket, "']'")?.span.end;
        Ok(Expr {
            kind: ExprKind::Vector(Box::new([x, y, z])),
            span: Span { start, end },
        })
    }

    fn finish_object(&mut self) -> Result<Expr, LangError> {
        let start = self.previous().span.start;
        let mut fields = Vec::new();

        while !self.at(TokenKind::RBrace) {
            if self.at(TokenKind::Eof) {
                return Err(self.expected("'}'"));
            }
            let field_start = self.current().span.start;
            let name = self.expect_ident("object field name")?;
            self.expect(TokenKind::Colon, "':'")?;
            let expr = self.parse_expr()?;
            let semi = self.expect(TokenKind::Semicolon, "';'")?;
            fields.push(ObjectField {
                name,
                expr,
                span: Span {
                    start: field_start,
                    end: semi.span.end,
                },
            });
        }

        let end = self.expect(TokenKind::RBrace, "'}'")?.span.end;
        Ok(Expr {
            kind: ExprKind::Object(fields),
            span: Span { start, end },
        })
    }

    fn current(&self) -> &Token {
        &self.tokens[self.idx]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.idx - 1]
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn matches(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, expected: &str) -> Result<Token, LangError> {
        if self.at(kind) {
            self.idx += 1;
            Ok(self.previous().clone())
        } else {
            Err(self.expected(expected))
        }
    }

    fn expect_ident(&mut self, expected: &str) -> Result<String, LangError> {
        if self.matches(TokenKind::Ident) {
            Ok(self.previous().lexeme.clone())
        } else {
            Err(self.expected(expected))
        }
    }

    fn expected(&self, expected: &str) -> LangError {
        let tok = self.current();
        LangError::syntax(
            tok.span,
            format!("expected {expected}, found '{}'", display_token(tok)),
        )
    }
}

fn display_token(token: &Token) -> String {
    if token.kind == TokenKind::Eof {
        "<eof>".to_owned()
    } else {
        token.lexeme.clone()
    }
}

fn unescape_string(raw: &str, span: Span) -> Result<String, LangError> {
    let mut out = String::new();
    let inner = &raw[1..raw.len() - 1];
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let Some(esc) = chars.next() else {
            return Err(LangError::syntax(span, "unterminated string escape"));
        };

        match esc {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            _ => {
                return Err(LangError::syntax(
                    span,
                    format!("invalid string escape \\{esc}"),
                ));
            }
        }
    }
    Ok(out)
}

pub fn parse_pcad(source: &str) -> Result<File, LangError> {
    let tokens = Lexer::new(source).tokenize()?;
    AstParser::new(tokens).parse_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_declarations_with_let_and_fields() {
        let src = r#"
solid body = box {
  w: 100;
  h: 50;
  d: 30;
}

feature hole1 = hole {
  let cx = body.w / 2;
  target: body.top;
  x: cx;
  y: body.h / 2;
  d: 5;
}
"#;

        let file = parse_pcad(src).expect("should parse");
        assert_eq!(file.decls.len(), 2);
        assert_eq!(file.decls[0].id, "body");
        assert_eq!(file.decls[1].id, "hole1");

        match &file.decls[1].entries[0] {
            Entry::Let { name, .. } => assert_eq!(name, "cx"),
            _ => panic!("expected let entry"),
        }
    }

    #[test]
    fn parses_expression_precedence() {
        let src = r#"
solid a = box {
  v: 1 + 2 * 3 == 7 && true || false;
}
"#;

        let file = parse_pcad(src).expect("should parse");
        let Entry::Field { expr, .. } = &file.decls[0].entries[0] else {
            panic!("expected field");
        };

        let ExprKind::Binary { op, .. } = &expr.kind else {
            panic!("expected binary");
        };
        assert_eq!(*op, BinaryOp::Or);
    }

    #[test]
    fn parses_object_and_vector_literals() {
        let src = r#"
feature ch1 = chamfer {
  target: { kind: "face"; of: body; normal: [0, 0, 1]; };
  dist: 2;
}
"#;

        let file = parse_pcad(src).expect("should parse");
        assert_eq!(file.decls.len(), 1);

        let Entry::Field { expr, .. } = &file.decls[0].entries[0] else {
            panic!("expected field");
        };

        let ExprKind::Object(fields) = &expr.kind else {
            panic!("expected object");
        };
        assert_eq!(fields.len(), 3);
    }

    #[test]
    fn supports_comments() {
        let src = r#"
// first
solid body = box {
  w: 1; /* inline */
  h: 2;
  d: 3;
}
"#;
        let file = parse_pcad(src).expect("should parse");
        assert_eq!(file.decls.len(), 1);
    }

    #[test]
    fn reports_syntax_error_on_missing_semicolon() {
        let src = r#"
solid body = box {
  w: 100
}
"#;
        let err = parse_pcad(src).expect_err("should fail");
        assert_eq!(err.code, ErrorCode::SyntaxError);
    }

    #[test]
    fn reports_duplicate_declaration_id() {
        let src = r#"
solid body = box {
  w: 1;
}
feature body = hole {
  target: body.top;
  x: 1;
  y: 1;
  d: 1;
}
"#;
        let err = parse_pcad(src).expect_err("should fail");
        assert_eq!(err.code, ErrorCode::DuplicateId);
    }

    #[test]
    fn parses_trailing_declaration_semicolon() {
        let src = r#"
solid body = box {
  w: 1;
};
"#;

        let file = parse_pcad(src).expect("should parse");
        assert_eq!(file.decls.len(), 1);
        assert_eq!(file.decls[0].id, "body");
    }
}

//! Parser: baut aus Tokens einen AST.
//!
//! Anweisungen werden per rekursivem Abstieg geparst, Ausdrücke mit einem
//! Pratt-Parser. Die Präzedenztabelle steht in [`Prec`] und [`infix_prec`].

use std::rc::Rc;

use crate::ast::{
    BinaryOp, Expr, ExprKind, FunctionDecl, Literal, LogicalOp, Stmt, StmtKind, UnaryOp,
};
use crate::error::{LumenError, Result};
use crate::token::{Span, Token, TokenKind};

/// Bindungsstärken, von schwach nach stark. Die Reihenfolge der Varianten
/// ist die Präzedenz (abgeleitetes `Ord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Lowest,
    /// `=` (rechtsassoziativ)
    Assignment,
    /// `||`
    Or,
    /// `&&`
    And,
    /// `==` `!=`
    Equality,
    /// `<` `<=` `>` `>=`
    Comparison,
    /// `+` `-`
    Term,
    /// `*` `/` `%`
    Factor,
    /// Präfix `-` `!`
    Unary,
    /// Aufruf `f(...)`
    Call,
}

impl Prec {
    /// Die nächsthöhere Stufe, für linksassoziative Operatoren.
    fn higher(self) -> Self {
        match self {
            Prec::Lowest => Prec::Assignment,
            Prec::Assignment => Prec::Or,
            Prec::Or => Prec::And,
            Prec::And => Prec::Equality,
            Prec::Equality => Prec::Comparison,
            Prec::Comparison => Prec::Term,
            Prec::Term => Prec::Factor,
            Prec::Factor => Prec::Unary,
            Prec::Unary | Prec::Call => Prec::Call,
        }
    }
}

/// Bindungsstärke eines Tokens in Infix-Position (`Lowest` = kein Infix-Operator).
fn infix_prec(kind: &TokenKind) -> Prec {
    match kind {
        TokenKind::Eq => Prec::Assignment,
        TokenKind::OrOr => Prec::Or,
        TokenKind::AndAnd => Prec::And,
        TokenKind::EqEq | TokenKind::BangEq => Prec::Equality,
        TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => Prec::Comparison,
        TokenKind::Plus | TokenKind::Minus => Prec::Term,
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Prec::Factor,
        TokenKind::LParen => Prec::Call,
        _ => Prec::Lowest,
    }
}

fn binary_op(kind: &TokenKind) -> Option<BinaryOp> {
    Some(match kind {
        TokenKind::Plus => BinaryOp::Add,
        TokenKind::Minus => BinaryOp::Sub,
        TokenKind::Star => BinaryOp::Mul,
        TokenKind::Slash => BinaryOp::Div,
        TokenKind::Percent => BinaryOp::Rem,
        TokenKind::EqEq => BinaryOp::Eq,
        TokenKind::BangEq => BinaryOp::NotEq,
        TokenKind::Lt => BinaryOp::Lt,
        TokenKind::LtEq => BinaryOp::LtEq,
        TokenKind::Gt => BinaryOp::Gt,
        TokenKind::GtEq => BinaryOp::GtEq,
        _ => return None,
    })
}

/// Parser über einem fertigen Tokenstrom.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Erzeugt einen Parser. Fehlt ein abschließendes `Eof`, wird es ergänzt,
    /// damit der Parser nie über das Ende hinaus liest.
    #[must_use]
    pub fn new(mut tokens: Vec<Token>) -> Self {
        if !matches!(tokens.last(), Some(t) if t.kind == TokenKind::Eof) {
            let span = tokens.last().map(|t| t.span).unwrap_or_default();
            tokens.push(Token {
                kind: TokenKind::Eof,
                span,
            });
        }
        Self { tokens, pos: 0 }
    }

    /// Parst ein ganzes Programm (eine Folge von Deklarationen und Anweisungen).
    pub fn parse_program(&mut self) -> Result<Vec<Stmt>> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        Ok(statements)
    }

    // ---- Token-Zugriff ----------------------------------------------------

    fn peek(&self) -> &Token {
        // `new` garantiert ein abschließendes Eof und `advance` geht nie darüber hinaus.
        &self.tokens[self.pos]
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    /// Verbraucht das aktuelle Token (Eof bleibt stehen). Der Clone ist nötig,
    /// weil Tokens Strings tragen und der Parser den Tokenstrom behält.
    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if !self.is_at_end() {
            self.pos += 1;
        }
        token
    }

    fn check(&self, kind: &TokenKind) -> bool {
        &self.peek().kind == kind
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, context: &str) -> Result<Token> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let found = &self.peek().kind;
            Err(LumenError::parse(
                format!("{context} erwartet, gefunden {found}"),
                self.peek().span,
            ))
        }
    }

    fn expect_ident(&mut self, context: &str) -> Result<(String, Span)> {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(name) => Ok((name, token.span)),
            other => Err(LumenError::parse(
                format!("{context} erwartet, gefunden {other}"),
                token.span,
            )),
        }
    }

    // ---- Anweisungen ------------------------------------------------------

    fn declaration(&mut self) -> Result<Stmt> {
        match self.peek().kind {
            TokenKind::Let => self.let_declaration(),
            TokenKind::Fn => self.fn_declaration(),
            _ => self.statement(),
        }
    }

    fn let_declaration(&mut self) -> Result<Stmt> {
        let start = self.advance().span;
        let (name, _) = self.expect_ident("Variablenname")?;
        let init = if self.eat(&TokenKind::Eq) {
            Some(self.expression()?)
        } else {
            None
        };
        let end = self
            .expect(&TokenKind::Semicolon, "';' nach Variablendeklaration")?
            .span;
        Ok(Stmt {
            kind: StmtKind::Let { name, init },
            span: start.merge(end),
        })
    }

    fn fn_declaration(&mut self) -> Result<Stmt> {
        let start = self.advance().span;
        let (name, _) = self.expect_ident("Funktionsname")?;
        self.expect(&TokenKind::LParen, "'(' nach dem Funktionsnamen")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let (param, _) = self.expect_ident("Parametername")?;
                params.push(param);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "')' nach den Parametern")?;

        let (body, block_span) = self.block_body()?;
        let span = start.merge(block_span);
        Ok(Stmt {
            kind: StmtKind::Function(Rc::new(FunctionDecl {
                name,
                params,
                body,
                span,
            })),
            span,
        })
    }

    fn statement(&mut self) -> Result<Stmt> {
        match self.peek().kind {
            TokenKind::Print => self.print_statement(),
            TokenKind::If => self.if_statement(),
            TokenKind::While => self.while_statement(),
            TokenKind::Return => self.return_statement(),
            TokenKind::LBrace => self.block_statement(),
            _ => self.expression_statement(),
        }
    }

    fn print_statement(&mut self) -> Result<Stmt> {
        let start = self.advance().span;
        let value = self.expression()?;
        let end = self
            .expect(&TokenKind::Semicolon, "';' nach 'print'")?
            .span;
        Ok(Stmt {
            kind: StmtKind::Print(value),
            span: start.merge(end),
        })
    }

    fn if_statement(&mut self) -> Result<Stmt> {
        let start = self.advance().span;
        let condition = self.expression()?;
        let then_branch = self.block_statement()?;

        let else_branch = if self.eat(&TokenKind::Else) {
            let branch = if self.check(&TokenKind::If) {
                self.if_statement()?
            } else {
                self.block_statement()?
            };
            Some(Box::new(branch))
        } else {
            None
        };

        let end = else_branch
            .as_ref()
            .map_or(then_branch.span, |branch| branch.span);
        Ok(Stmt {
            kind: StmtKind::If {
                condition,
                then_branch: Box::new(then_branch),
                else_branch,
            },
            span: start.merge(end),
        })
    }

    fn while_statement(&mut self) -> Result<Stmt> {
        let start = self.advance().span;
        let condition = self.expression()?;
        let body = self.block_statement()?;
        let span = start.merge(body.span);
        Ok(Stmt {
            kind: StmtKind::While {
                condition,
                body: Box::new(body),
            },
            span,
        })
    }

    fn return_statement(&mut self) -> Result<Stmt> {
        let start = self.advance().span;
        let value = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        let end = self
            .expect(&TokenKind::Semicolon, "';' nach 'return'")?
            .span;
        Ok(Stmt {
            kind: StmtKind::Return(value),
            span: start.merge(end),
        })
    }

    fn block_statement(&mut self) -> Result<Stmt> {
        let (statements, span) = self.block_body()?;
        Ok(Stmt {
            kind: StmtKind::Block(statements),
            span,
        })
    }

    /// Parst `{ ... }` und liefert die Anweisungen sowie den Span des ganzen Blocks.
    fn block_body(&mut self) -> Result<(Vec<Stmt>, Span)> {
        let start = self.expect(&TokenKind::LBrace, "'{'")?.span;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        let end = self
            .expect(&TokenKind::RBrace, "'}' am Ende des Blocks")?
            .span;
        Ok((statements, start.merge(end)))
    }

    fn expression_statement(&mut self) -> Result<Stmt> {
        let expr = self.expression()?;
        let end = self
            .expect(&TokenKind::Semicolon, "';' nach dem Ausdruck")?
            .span;
        let span = expr.span.merge(end);
        Ok(Stmt {
            kind: StmtKind::Expr(expr),
            span,
        })
    }

    // ---- Ausdrücke (Pratt) ------------------------------------------------

    fn expression(&mut self) -> Result<Expr> {
        self.parse_precedence(Prec::Lowest)
    }

    /// Kern des Pratt-Parsers: parst einen Präfix-Ausdruck und frisst danach
    /// alle Infix-Operatoren, die mindestens so stark binden wie `min`.
    fn parse_precedence(&mut self, min: Prec) -> Result<Expr> {
        let mut left = self.parse_prefix()?;
        loop {
            let prec = infix_prec(&self.peek().kind);
            if prec == Prec::Lowest || prec < min {
                break;
            }
            let operator = self.advance();
            left = self.parse_infix(left, &operator, prec)?;
        }
        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<Expr> {
        let token = self.advance();
        let span = token.span;
        let kind = match token.kind {
            TokenKind::Int(v) => ExprKind::Literal(Literal::Int(v)),
            TokenKind::Float(v) => ExprKind::Literal(Literal::Float(v)),
            TokenKind::Str(s) => ExprKind::Literal(Literal::Str(Rc::from(s))),
            TokenKind::True => ExprKind::Literal(Literal::Bool(true)),
            TokenKind::False => ExprKind::Literal(Literal::Bool(false)),
            TokenKind::Nil => ExprKind::Literal(Literal::Nil),
            TokenKind::Ident(name) => ExprKind::Variable(name),
            TokenKind::LParen => {
                let inner = self.parse_precedence(Prec::Lowest)?;
                self.expect(&TokenKind::RParen, "')' nach dem Ausdruck")?;
                return Ok(inner);
            }
            TokenKind::Minus => return self.finish_unary(UnaryOp::Neg, span),
            TokenKind::Bang => return self.finish_unary(UnaryOp::Not, span),
            other => {
                return Err(LumenError::parse(
                    format!("Ausdruck erwartet, gefunden {other}"),
                    span,
                ))
            }
        };
        Ok(Expr { kind, span })
    }

    fn finish_unary(&mut self, op: UnaryOp, start: Span) -> Result<Expr> {
        let operand = self.parse_precedence(Prec::Unary)?;
        let span = start.merge(operand.span);
        Ok(Expr {
            kind: ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            span,
        })
    }

    fn parse_infix(&mut self, left: Expr, operator: &Token, prec: Prec) -> Result<Expr> {
        match &operator.kind {
            TokenKind::LParen => self.finish_call(left),
            TokenKind::Eq => {
                // Rechtsassoziativ: rechts mit gleicher Stufe weiterparsen.
                let value = self.parse_precedence(Prec::Assignment)?;
                let span = left.span.merge(value.span);
                match left.kind {
                    ExprKind::Variable(name) => Ok(Expr {
                        kind: ExprKind::Assign {
                            name,
                            value: Box::new(value),
                        },
                        span,
                    }),
                    _ => Err(LumenError::parse("Ungültiges Zuweisungsziel", left.span)),
                }
            }
            TokenKind::OrOr => self.finish_logical(left, LogicalOp::Or, prec),
            TokenKind::AndAnd => self.finish_logical(left, LogicalOp::And, prec),
            other => {
                let Some(op) = binary_op(other) else {
                    return Err(LumenError::parse(
                        format!("Unerwarteter Operator {other}"),
                        operator.span,
                    ));
                };
                // Linksassoziativ: rechts nur stärker bindende Operatoren fressen.
                let right = self.parse_precedence(prec.higher())?;
                let span = left.span.merge(right.span);
                Ok(Expr {
                    kind: ExprKind::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                })
            }
        }
    }

    fn finish_logical(&mut self, left: Expr, op: LogicalOp, prec: Prec) -> Result<Expr> {
        let right = self.parse_precedence(prec.higher())?;
        let span = left.span.merge(right.span);
        Ok(Expr {
            kind: ExprKind::Logical {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        })
    }

    fn finish_call(&mut self, callee: Expr) -> Result<Expr> {
        let mut args = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                args.push(self.expression()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        let end = self
            .expect(&TokenKind::RParen, "')' nach den Argumenten")?
            .span;
        let span = callee.span.merge(end);
        Ok(Expr {
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
            span,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse(source: &str) -> Result<Vec<Stmt>> {
        Parser::new(tokenize(source)?).parse_program()
    }

    fn first_expr(source: &str) -> Expr {
        let mut program = parse(source).expect("Programm sollte parsen");
        match program.remove(0).kind {
            StmtKind::Expr(expr) => expr,
            other => panic!("Ausdrucksanweisung erwartet, erhielt {other:?}"),
        }
    }

    fn parse_error(source: &str) -> String {
        match parse(source) {
            Err(LumenError::Parse { message, .. }) => message,
            other => panic!("Syntaxfehler erwartet, erhielt {other:?}"),
        }
    }

    #[test]
    fn parser_works_without_the_lexer() {
        let tok = |kind| Token {
            kind,
            span: Span::default(),
        };
        let tokens = vec![
            tok(TokenKind::Int(1)),
            tok(TokenKind::Plus),
            tok(TokenKind::Int(2)),
            tok(TokenKind::Semicolon),
        ];
        let program = Parser::new(tokens).parse_program().expect("sollte parsen");
        assert_eq!(program.len(), 1);
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        let expr = first_expr("1 + 2 * 3;");
        let ExprKind::Binary { op, right, .. } = expr.kind else {
            panic!("Binary erwartet");
        };
        assert_eq!(op, BinaryOp::Add);
        assert!(matches!(
            right.kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }

    #[test]
    fn grouping_overrides_precedence() {
        let expr = first_expr("(1 + 2) * 3;");
        let ExprKind::Binary { op, left, .. } = expr.kind else {
            panic!("Binary erwartet");
        };
        assert_eq!(op, BinaryOp::Mul);
        assert!(matches!(
            left.kind,
            ExprKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn subtraction_is_left_associative() {
        let expr = first_expr("1 - 2 - 3;");
        let ExprKind::Binary { left, right, .. } = expr.kind else {
            panic!("Binary erwartet");
        };
        assert!(matches!(
            left.kind,
            ExprKind::Binary {
                op: BinaryOp::Sub,
                ..
            }
        ));
        assert!(matches!(right.kind, ExprKind::Literal(Literal::Int(3))));
    }

    #[test]
    fn assignment_is_right_associative() {
        let expr = first_expr("a = b = 1;");
        let ExprKind::Assign { name, value } = expr.kind else {
            panic!("Assign erwartet");
        };
        assert_eq!(name, "a");
        assert!(matches!(value.kind, ExprKind::Assign { .. }));
    }

    #[test]
    fn logical_or_binds_weaker_than_and() {
        let expr = first_expr("a || b && c;");
        let ExprKind::Logical { op, right, .. } = expr.kind else {
            panic!("Logical erwartet");
        };
        assert_eq!(op, LogicalOp::Or);
        assert!(matches!(
            right.kind,
            ExprKind::Logical {
                op: LogicalOp::And,
                ..
            }
        ));
    }

    #[test]
    fn unary_binds_tighter_than_binary_but_weaker_than_call() {
        let expr = first_expr("-a * f(1);");
        let ExprKind::Binary { left, right, .. } = expr.kind else {
            panic!("Binary erwartet");
        };
        assert!(matches!(
            left.kind,
            ExprKind::Unary {
                op: UnaryOp::Neg,
                ..
            }
        ));
        assert!(matches!(right.kind, ExprKind::Call { .. }));
    }

    #[test]
    fn calls_with_multiple_arguments_and_chaining() {
        let expr = first_expr("f(1, 2)(3);");
        let ExprKind::Call { callee, args } = expr.kind else {
            panic!("Call erwartet");
        };
        assert_eq!(args.len(), 1);
        assert!(matches!(callee.kind, ExprKind::Call { .. }));
    }

    #[test]
    fn function_declaration() {
        let program = parse("fn add(a, b) { return a + b; }").expect("sollte parsen");
        let StmtKind::Function(decl) = &program[0].kind else {
            panic!("Function erwartet");
        };
        assert_eq!(decl.name, "add");
        assert_eq!(decl.params, vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(decl.body.len(), 1);
    }

    #[test]
    fn if_else_if_else_chain() {
        let program =
            parse("if a { x; } else if b { y; } else { z; }").expect("sollte parsen");
        let StmtKind::If { else_branch, .. } = &program[0].kind else {
            panic!("If erwartet");
        };
        let nested = else_branch.as_ref().expect("else erwartet");
        assert!(matches!(nested.kind, StmtKind::If { .. }));
    }

    #[test]
    fn spans_cover_the_whole_expression() {
        let source = "let x = 1 + 22;";
        let program = parse(source).expect("sollte parsen");
        let StmtKind::Let { init, .. } = &program[0].kind else {
            panic!("Let erwartet");
        };
        let span = init.as_ref().expect("Initialisierer erwartet").span;
        assert_eq!(&source[span.start..span.end], "1 + 22");
    }

    #[test]
    fn missing_variable_name() {
        assert!(parse_error("let = 5;").contains("Variablenname"));
    }

    #[test]
    fn missing_operand() {
        assert!(parse_error("1 + ;").contains("Ausdruck erwartet"));
    }

    #[test]
    fn missing_closing_paren() {
        assert!(parse_error("(1 + 2;").contains("')'"));
    }

    #[test]
    fn missing_semicolon_reports_what_was_found() {
        let message = parse_error("print 1 print 2;");
        assert!(message.contains("';'"));
        assert!(message.contains("print"));
    }

    #[test]
    fn invalid_assignment_target() {
        assert!(parse_error("1 = 2;").contains("Zuweisungsziel"));
        assert!(parse_error("a + b = 2;").contains("Zuweisungsziel"));
    }

    #[test]
    fn unclosed_block_is_an_error() {
        assert!(parse_error("{ print 1;").contains("'}'"));
    }

    #[test]
    fn if_requires_a_block() {
        assert!(parse_error("if true print 1;").contains("'{'"));
    }

    #[test]
    fn error_span_points_to_the_offending_token() {
        match parse("let x = ;") {
            Err(LumenError::Parse { span, .. }) => assert_eq!((span.line, span.column), (1, 9)),
            other => panic!("Syntaxfehler erwartet, erhielt {other:?}"),
        }
    }
}

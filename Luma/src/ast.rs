//! Abstrakter Syntaxbaum (AST).
//!
//! Der AST kennt keine Auswertung. Ein Tree-Walking-Interpreter läuft direkt
//! darüber; ein späterer Bytecode-Compiler würde denselben Baum konsumieren.

use std::rc::Rc;

use crate::token::Span;

/// Ein Ausdruck samt Quellposition.
#[derive(Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

/// Die Ausdrucksarten der Sprache.
#[derive(Debug, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    Variable(String),
    Assign {
        name: String,
        value: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `&&` und `||` sind getrennt von `Binary`, da sie kurzschließen.
    Logical {
        op: LogicalOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
}

/// Literalwerte. Strings sind `Rc<str>`, damit die Auswertung sie ohne Kopie teilen kann.
#[derive(Debug, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Bool(bool),
    Nil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

impl BinaryOp {
    /// Das Operatorsymbol, wie es im Quelltext steht (für Fehlermeldungen).
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::NotEq => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::LtEq => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::GtEq => ">=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}

/// Eine Anweisung samt Quellposition.
#[derive(Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

/// Die Anweisungsarten der Sprache.
#[derive(Debug, PartialEq)]
pub enum StmtKind {
    Let {
        name: String,
        init: Option<Expr>,
    },
    Expr(Expr),
    Print(Expr),
    Block(Vec<Stmt>),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    /// `Rc`, weil Funktionswerte zur Laufzeit die Deklaration teilen, ohne den AST zu kopieren.
    Function(Rc<FunctionDecl>),
    Return(Option<Expr>),
}

/// Deklaration einer Funktion.
#[derive(Debug, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

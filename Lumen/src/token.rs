//! Token-Definitionen und Quelltext-Positionen.

use std::fmt;

/// Ein Abschnitt des Quelltexts.
///
/// `start`/`end` sind Byte-Offsets (für Ausschnitte), `line`/`column` sind
/// 1-basiert und zeigen auf den Beginn des Abschnitts (für Fehlermeldungen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

impl Span {
    /// Verbindet zwei Spans zu einem, der von `self` bis `other` reicht.
    #[must_use]
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
            line: self.line,
            column: self.column,
        }
    }
}

/// Die Art eines Tokens (inklusive Nutzdaten bei Literalen und Bezeichnern).
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literale und Bezeichner
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),

    // Schlüsselwörter
    Let,
    Fn,
    If,
    Else,
    While,
    Return,
    Print,
    True,
    False,
    Nil,

    // Operatoren
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    BangEq,
    Eq,
    EqEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,

    // Interpunktion
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semicolon,

    /// Ende der Eingabe.
    Eof,
}

impl TokenKind {
    /// Quelltext-Darstellung für Token mit fester Schreibweise.
    fn lexeme(&self) -> &'static str {
        match self {
            TokenKind::Let => "let",
            TokenKind::Fn => "fn",
            TokenKind::If => "if",
            TokenKind::Else => "else",
            TokenKind::While => "while",
            TokenKind::Return => "return",
            TokenKind::Print => "print",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::Nil => "nil",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Bang => "!",
            TokenKind::BangEq => "!=",
            TokenKind::Eq => "=",
            TokenKind::EqEq => "==",
            TokenKind::Lt => "<",
            TokenKind::LtEq => "<=",
            TokenKind::Gt => ">",
            TokenKind::GtEq => ">=",
            TokenKind::AndAnd => "&&",
            TokenKind::OrOr => "||",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::Comma => ",",
            TokenKind::Semicolon => ";",
            TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Str(_)
            | TokenKind::Ident(_)
            | TokenKind::Eof => "",
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Int(n) => write!(f, "Zahl {n}"),
            TokenKind::Float(x) => write!(f, "Zahl {x}"),
            TokenKind::Str(_) => write!(f, "String"),
            TokenKind::Ident(name) => write!(f, "Bezeichner '{name}'"),
            TokenKind::Eof => write!(f, "Dateiende"),
            other => write!(f, "'{}'", other.lexeme()),
        }
    }
}

/// Ein Token samt Position im Quelltext.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

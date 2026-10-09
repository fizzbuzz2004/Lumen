//! Lexer: zerlegt Quelltext in [`Token`]s.
//!
//! Der Lexer kennt weder Grammatik noch Auswertung und ist damit unabhängig
//! vom Parser testbar.

use crate::error::{LumenError, Result};
use crate::token::{Span, Token, TokenKind};

/// Komfortfunktion: tokenisiert den gesamten Quelltext.
///
/// Das letzte Token ist immer [`TokenKind::Eof`].
pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    Lexer::new(source).tokenize()
}

/// Zeichenweiser Lexer über einem Quelltext.
pub struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    line: u32,
    column: u32,
}

impl<'a> Lexer<'a> {
    /// Erzeugt einen Lexer am Anfang des Quelltexts.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    /// Liest alle Tokens bis einschließlich [`TokenKind::Eof`].
    pub fn tokenize(mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let done = token.kind == TokenKind::Eof;
            tokens.push(token);
            if done {
                return Ok(tokens);
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        self.source[self.pos..].chars().nth(1)
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn take_while(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some(c) = self.peek() {
            if !predicate(c) {
                break;
            }
            self.advance();
        }
    }

    fn span_from(&self, start: usize, line: u32, column: u32) -> Span {
        Span {
            start,
            end: self.pos,
            line,
            column,
        }
    }

    fn error(&self, message: impl Into<String>, start: usize, line: u32, column: u32) -> LumenError {
        LumenError::lex(message, self.span_from(start, line, column))
    }

    /// Überspringt Leerraum und `//`-Kommentare.
    fn skip_trivia(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else if ch == '/' && self.peek_next() == Some('/') {
                self.take_while(|c| c != '\n');
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token> {
        self.skip_trivia();
        let (start, line, column) = (self.pos, self.line, self.column);

        let Some(ch) = self.advance() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                span: self.span_from(start, line, column),
            });
        };

        let kind = match ch {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '!' if self.eat('=') => TokenKind::BangEq,
            '!' => TokenKind::Bang,
            '=' if self.eat('=') => TokenKind::EqEq,
            '=' => TokenKind::Eq,
            '<' if self.eat('=') => TokenKind::LtEq,
            '<' => TokenKind::Lt,
            '>' if self.eat('=') => TokenKind::GtEq,
            '>' => TokenKind::Gt,
            '&' if self.eat('&') => TokenKind::AndAnd,
            '|' if self.eat('|') => TokenKind::OrOr,
            '&' | '|' => {
                return Err(self.error(
                    format!("Einzelnes '{ch}' ist ungültig, meintest du '{ch}{ch}'?"),
                    start,
                    line,
                    column,
                ))
            }
            '"' => self.string(start, line, column)?,
            c if c.is_ascii_digit() => self.number(start, line, column)?,
            c if c.is_ascii_alphabetic() || c == '_' => self.identifier(start),
            other => {
                return Err(self.error(
                    format!("Unerwartetes Zeichen '{other}'"),
                    start,
                    line,
                    column,
                ))
            }
        };

        Ok(Token {
            kind,
            span: self.span_from(start, line, column),
        })
    }

    fn number(&mut self, start: usize, line: u32, column: u32) -> Result<TokenKind> {
        self.take_while(|c| c.is_ascii_digit());

        let mut is_float = false;
        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            self.advance();
            self.take_while(|c| c.is_ascii_digit());
        }

        let text = &self.source[start..self.pos];
        if is_float {
            text.parse::<f64>()
                .map(TokenKind::Float)
                .map_err(|_| self.error(format!("Ungültige Zahl '{text}'"), start, line, column))
        } else {
            text.parse::<i64>().map(TokenKind::Int).map_err(|_| {
                self.error(
                    format!("Ganzzahl '{text}' ist zu groß für i64"),
                    start,
                    line,
                    column,
                )
            })
        }
    }

    fn string(&mut self, start: usize, line: u32, column: u32) -> Result<TokenKind> {
        let mut value = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(self.error("Nicht abgeschlossener String", start, line, column));
                }
                Some('"') => return Ok(TokenKind::Str(value)),
                Some('\\') => {
                    let escaped = match self.advance() {
                        Some('n') => '\n',
                        Some('t') => '\t',
                        Some('r') => '\r',
                        Some('"') => '"',
                        Some('\\') => '\\',
                        Some(other) => {
                            return Err(self.error(
                                format!("Unbekannte Escape-Sequenz '\\{other}'"),
                                start,
                                line,
                                column,
                            ));
                        }
                        None => {
                            return Err(self.error(
                                "Nicht abgeschlossener String",
                                start,
                                line,
                                column,
                            ));
                        }
                    };
                    value.push(escaped);
                }
                Some(c) => value.push(c),
            }
        }
    }

    fn identifier(&mut self, start: usize) -> TokenKind {
        self.take_while(|c| c.is_ascii_alphanumeric() || c == '_');
        match &self.source[start..self.pos] {
            "let" => TokenKind::Let,
            "fn" => TokenKind::Fn,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "print" => TokenKind::Print,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "nil" => TokenKind::Nil,
            name => TokenKind::Ident(name.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source)
            .expect("Quelltext sollte tokenisierbar sein")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn empty_input_yields_only_eof() {
        assert_eq!(kinds(""), vec![TokenKind::Eof]);
        assert_eq!(kinds("  \n\t "), vec![TokenKind::Eof]);
    }

    #[test]
    fn single_and_double_char_operators() {
        assert_eq!(
            kinds("+ - * / % ! != = == < <= > >= && ||"),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::Bang,
                TokenKind::BangEq,
                TokenKind::Eq,
                TokenKind::EqEq,
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
                TokenKind::AndAnd,
                TokenKind::OrOr,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn integers_and_floats() {
        assert_eq!(
            kinds("42 3.25 0"),
            vec![
                TokenKind::Int(42),
                TokenKind::Float(3.25),
                TokenKind::Int(0),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn keywords_and_identifiers() {
        assert_eq!(
            kinds("let letter fn_x nil true"),
            vec![
                TokenKind::Let,
                TokenKind::Ident("letter".into()),
                TokenKind::Ident("fn_x".into()),
                TokenKind::Nil,
                TokenKind::True,
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn string_literals_with_escapes_and_unicode() {
        assert_eq!(
            kinds(r#""a\nb\"c" "Grüße""#),
            vec![
                TokenKind::Str("a\nb\"c".into()),
                TokenKind::Str("Grüße".into()),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(
            kinds("1 // Kommentar\n2"),
            vec![TokenKind::Int(1), TokenKind::Int(2), TokenKind::Eof]
        );
    }

    #[test]
    fn tracks_line_and_column() {
        let source = "let x\n  y";
        let tokens = tokenize(source).expect("sollte lexen");
        let positions: Vec<(u32, u32)> = tokens
            .iter()
            .map(|t| (t.span.line, t.span.column))
            .collect();
        assert_eq!(positions, vec![(1, 1), (1, 5), (2, 3), (2, 4)]);
        assert_eq!(&source[tokens[2].span.start..tokens[2].span.end], "y");
    }

    #[test]
    fn unterminated_string_is_an_error_at_the_opening_quote() {
        let err = tokenize("let s = \"offen").unwrap_err();
        match err {
            LumenError::Lex { span, message } => {
                assert_eq!((span.line, span.column), (1, 9));
                assert!(message.contains("Nicht abgeschlossen"));
            }
            other => panic!("Lexer-Fehler erwartet, erhielt {other:?}"),
        }
    }

    #[test]
    fn unknown_escape_sequence_is_an_error() {
        assert!(matches!(tokenize(r#""\q""#), Err(LumenError::Lex { .. })));
    }

    #[test]
    fn unexpected_character_is_an_error() {
        let err = tokenize("1 + @").unwrap_err();
        match err {
            LumenError::Lex { span, message } => {
                assert_eq!(span.column, 5);
                assert!(message.contains('@'));
            }
            other => panic!("Lexer-Fehler erwartet, erhielt {other:?}"),
        }
    }

    #[test]
    fn lone_ampersand_and_pipe_are_errors() {
        assert!(matches!(tokenize("a & b"), Err(LumenError::Lex { .. })));
        assert!(matches!(tokenize("a | b"), Err(LumenError::Lex { .. })));
    }

    #[test]
    fn integer_overflow_is_an_error() {
        assert!(matches!(
            tokenize("99999999999999999999"),
            Err(LumenError::Lex { .. })
        ));
    }
}

//! Fehlertypen für alle Phasen (Lexer, Parser, Laufzeit).

use thiserror::Error;

use crate::token::Span;

/// Bequemer Alias für Ergebnisse dieser Crate.
pub type Result<T> = std::result::Result<T, LumenError>;

/// Alle Fehler, die beim Ausführen von Lumen-Code auftreten können.
///
/// Jede Variante außer `Io` trägt eine [`Span`], damit Zeile und Spalte
/// angezeigt und der betroffene Quelltext markiert werden können.
#[derive(Debug, Error)]
pub enum LumenError {
    /// Ungültige Zeichen oder Literale.
    #[error("Lexer-Fehler bei {}:{}: {message}", .span.line, .span.column)]
    Lex { message: String, span: Span },

    /// Der Tokenstrom entspricht nicht der Grammatik.
    #[error("Syntaxfehler bei {}:{}: {message}", .span.line, .span.column)]
    Parse { message: String, span: Span },

    /// Fehler während der Ausführung (Typfehler, undefinierte Variablen, ...).
    #[error("Laufzeitfehler bei {}:{}: {message}", .span.line, .span.column)]
    Runtime { message: String, span: Span },

    /// Fehler beim Lesen oder Schreiben.
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
}

impl LumenError {
    /// Erzeugt einen Lexer-Fehler.
    pub fn lex(message: impl Into<String>, span: Span) -> Self {
        Self::Lex {
            message: message.into(),
            span,
        }
    }

    /// Erzeugt einen Parser-Fehler.
    pub fn parse(message: impl Into<String>, span: Span) -> Self {
        Self::Parse {
            message: message.into(),
            span,
        }
    }

    /// Erzeugt einen Laufzeitfehler.
    pub fn runtime(message: impl Into<String>, span: Span) -> Self {
        Self::Runtime {
            message: message.into(),
            span,
        }
    }

    /// Die Quellposition des Fehlers, falls vorhanden.
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        match self {
            Self::Lex { span, .. } | Self::Parse { span, .. } | Self::Runtime { span, .. } => {
                Some(*span)
            }
            Self::Io(_) => None,
        }
    }

    /// Formatiert den Fehler mit betroffener Quelltextzeile und Markierung:
    ///
    /// ```text
    /// Laufzeitfehler bei 2:7: Undefinierte Variable 'y'
    ///   |
    /// 2 | print y;
    ///   |       ^
    /// ```
    #[must_use]
    pub fn render(&self, source: &str) -> String {
        let header = self.to_string();
        let Some(span) = self.span() else {
            return header;
        };

        let line_no = span.line as usize;
        let line_text = source
            .lines()
            .nth(line_no.saturating_sub(1))
            .unwrap_or_default();
        let column = (span.column as usize).saturating_sub(1);
        let available = line_text.chars().count().saturating_sub(column).max(1);
        let width = source
            .get(span.start..span.end)
            .map_or(1, |s| s.lines().next().unwrap_or("").chars().count())
            .clamp(1, available);

        let gutter = " ".repeat(line_no.to_string().len());
        format!(
            "{header}\n{gutter} |\n{line_no} | {line_text}\n{gutter} | {}{}",
            " ".repeat(column),
            "^".repeat(width)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_contains_line_and_column() {
        let span = Span {
            start: 6,
            end: 7,
            line: 2,
            column: 7,
        };
        let err = LumenError::runtime("kaputt", span);
        assert_eq!(err.to_string(), "Laufzeitfehler bei 2:7: kaputt");
    }

    #[test]
    fn render_marks_the_offending_source() {
        let source = "print 1;\nprint y;";
        let span = Span {
            start: 15,
            end: 16,
            line: 2,
            column: 7,
        };
        let rendered = LumenError::runtime("Undefinierte Variable 'y'", span).render(source);
        assert!(rendered.contains("2 | print y;"));
        assert!(rendered.ends_with("      ^"));
    }

    #[test]
    fn render_survives_out_of_range_spans() {
        let span = Span {
            start: 500,
            end: 600,
            line: 99,
            column: 40,
        };
        let rendered = LumenError::parse("x", span).render("kurz");
        assert!(rendered.contains('^'));
    }
}

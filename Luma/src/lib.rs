//! # Lumen
//!
//! Eine kleine, dynamisch typisierte Sprache mit Tree-Walking-Interpreter.
//!
//! Pipeline: Quelltext → [`lexer`] → Tokens → [`parser`] → [`ast`] → [`interpreter`].
//!
//! ```
//! let output = lumen::run_to_string("print 1 + 2;").unwrap();
//! assert_eq!(output, "3\n");
//! ```

pub mod ast;
pub mod environment;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod value;

use std::io::Write;

use crate::ast::Stmt;
use crate::interpreter::Interpreter;
use crate::parser::Parser;

pub use crate::error::{LumenError, Result};

/// Tokenisiert und parst Quelltext zu einem Programm (Liste von Anweisungen).
pub fn parse_source(source: &str) -> Result<Vec<Stmt>> {
    let tokens = lexer::tokenize(source)?;
    Parser::new(tokens).parse_program()
}

/// Führt Quelltext aus. Die Ausgabe von `print` wird nach `out` geschrieben.
pub fn run_source<W: Write>(source: &str, out: &mut W) -> Result<()> {
    let program = parse_source(source)?;
    Interpreter::new(out).run(&program)
}

/// Führt Quelltext aus und gibt die gesamte `print`-Ausgabe als String zurück.
pub fn run_to_string(source: &str) -> Result<String> {
    let mut buffer = Vec::new();
    run_source(source, &mut buffer)?;
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

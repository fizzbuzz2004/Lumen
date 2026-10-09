//! Laufzeitwerte.

use std::fmt;
use std::rc::Rc;

use crate::ast::FunctionDecl;
use crate::environment::EnvRef;

/// Ein Wert, wie ihn der Interpreter zur Laufzeit verarbeitet.
///
/// Strings und Funktionen liegen hinter `Rc`, sodass `clone()` billig ist.
#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Function(Rc<Function>),
}

/// Eine benutzerdefinierte Funktion samt dem Scope, in dem sie definiert wurde (Closure).
pub struct Function {
    pub decl: Rc<FunctionDecl>,
    pub closure: EnvRef,
}

// Manuelles Debug: Der Closure-Scope kann die Funktion selbst enthalten, ein
// abgeleitetes Debug würde endlos rekursieren.
impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Function")
            .field("name", &self.decl.name)
            .finish_non_exhaustive()
    }
}

impl Value {
    /// Wahrheitswert: nur `nil` und `false` sind falsch.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    /// Typname für Fehlermeldungen.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::Function(_) => "function",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            // Gemischte Zahlen werden numerisch verglichen: 1 == 1.0
            (Value::Int(i), Value::Float(x)) | (Value::Float(x), Value::Int(i)) => {
                (*i as f64) == *x
            }
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            // `{:?}` behält bei ganzzahligen Floats das ".0" (3.0 statt 3).
            Value::Float(x) => write!(f, "{x:?}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Function(func) => write!(f, "<fn {}>", func.decl.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truthiness() {
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(Value::Int(0).is_truthy());
        assert!(Value::Str(Rc::from("")).is_truthy());
    }

    #[test]
    fn equality_across_numeric_types() {
        assert_eq!(Value::Int(1), Value::Float(1.0));
        assert_ne!(Value::Int(1), Value::Bool(true));
        assert_ne!(Value::Nil, Value::Bool(false));
    }

    #[test]
    fn display_formats() {
        assert_eq!(Value::Float(3.0).to_string(), "3.0");
        assert_eq!(Value::Float(2.5).to_string(), "2.5");
        assert_eq!(Value::Nil.to_string(), "nil");
        assert_eq!(Value::Str(Rc::from("hi")).to_string(), "hi");
    }
}

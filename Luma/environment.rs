//! Verkettete Gültigkeitsbereiche (Scopes).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::Value;

/// Geteilter, veränderbarer Verweis auf einen Scope.
///
/// **Warum `Rc<RefCell<..>>`?** Closures müssen den Scope, in dem sie
/// definiert wurden, über dessen Lebenszeit hinaus behalten, und mehrere
/// Funktionen können denselben Scope teilen und verändern. Das ist geteilter
/// Besitz mit innerer Veränderbarkeit. Die Alternative wäre eine Arena mit
/// Scope-Indizes: kein Referenzzählen und keine Zyklen, aber aufwendiger.
/// Für einen Lern-Interpreter ist `Rc<RefCell<..>>` die lesbarere Wahl.
///
/// **Bekannte Einschränkung:** Eine Funktion, die in dem Scope gespeichert
/// ist, den sie selbst einfängt, bildet einen Referenzzyklus und wird erst
/// beim Programmende freigegeben (Leck). Eine Arena oder ein Tracing-GC
/// würde das beheben.
pub type EnvRef = Rc<RefCell<Environment>>;

/// Ein Scope mit Variablen und optionalem äußeren Scope.
#[derive(Debug, Default)]
pub struct Environment {
    values: HashMap<String, Value>,
    enclosing: Option<EnvRef>,
}

impl Environment {
    /// Erzeugt den globalen Scope.
    #[must_use]
    pub fn new_global() -> EnvRef {
        Rc::new(RefCell::new(Self::default()))
    }

    /// Erzeugt einen inneren Scope, der `enclosing` umschließt.
    #[must_use]
    pub fn with_enclosing(enclosing: EnvRef) -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            enclosing: Some(enclosing),
        }))
    }

    /// Legt eine Variable im aktuellen Scope an (überschreibt eine gleichnamige dort).
    pub fn define(&mut self, name: &str, value: Value) {
        self.values.insert(name.to_owned(), value);
    }

    /// Sucht eine Variable vom inneren zum äußeren Scope.
    ///
    /// Liefert eine Kopie des Werts. Das ist billig, da `Value` Strings und
    /// Funktionen per `Rc` teilt.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Value> {
        self.values.get(name).cloned().or_else(|| {
            self.enclosing
                .as_ref()
                .and_then(|outer| outer.borrow().get(name))
        })
    }

    /// Weist einer bestehenden Variable einen neuen Wert zu.
    /// Gibt `false` zurück, wenn sie in keinem Scope existiert.
    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if let Some(slot) = self.values.get_mut(name) {
            *slot = value;
            true
        } else if let Some(outer) = &self.enclosing {
            outer.borrow_mut().assign(name, value)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn define_and_get() {
        let env = Environment::new_global();
        env.borrow_mut().define("x", Value::Int(1));
        assert_eq!(env.borrow().get("x"), Some(Value::Int(1)));
        assert_eq!(env.borrow().get("y"), None);
    }

    #[test]
    fn lookup_walks_the_chain_and_inner_shadows_outer() {
        let outer = Environment::new_global();
        outer.borrow_mut().define("x", Value::Int(1));
        let inner = Environment::with_enclosing(Rc::clone(&outer));
        assert_eq!(inner.borrow().get("x"), Some(Value::Int(1)));

        inner.borrow_mut().define("x", Value::Int(2));
        assert_eq!(inner.borrow().get("x"), Some(Value::Int(2)));
        assert_eq!(outer.borrow().get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn assign_updates_the_defining_scope() {
        let outer = Environment::new_global();
        outer.borrow_mut().define("x", Value::Int(1));
        let inner = Environment::with_enclosing(Rc::clone(&outer));

        assert!(inner.borrow_mut().assign("x", Value::Int(5)));
        assert_eq!(outer.borrow().get("x"), Some(Value::Int(5)));
        assert!(!inner.borrow_mut().assign("missing", Value::Nil));
    }
}

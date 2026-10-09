//! Tree-Walking-Interpreter: führt den AST direkt aus.

use std::io::Write;
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, ExprKind, Literal, LogicalOp, Stmt, StmtKind, UnaryOp};
use crate::environment::{EnvRef, Environment};
use crate::error::{LumenError, Result};
use crate::token::Span;
use crate::value::{Function, Value};

/// Standardlimit für verschachtelte Funktionsaufrufe. Schützt vor Stack-Überlauf
/// bei unendlicher Rekursion; `main` startet den Interpreter mit großem Stack.
pub const DEFAULT_MAX_CALL_DEPTH: usize = 1000;

/// Wie eine Anweisung endet: normal oder durch `return`.
///
/// `return` ist kein Fehler, sondern normaler Kontrollfluss. Ein eigener
/// Rückgabetyp ist sauberer, als dafür `Err` zu missbrauchen.
enum Flow {
    Normal,
    Return(Value),
}

/// Der Interpreter. Die Ausgabe von `print` geht an `out`, damit Tests sie
/// abfangen können.
pub struct Interpreter<W: Write> {
    env: EnvRef,
    out: W,
    depth: usize,
    max_depth: usize,
}

impl<W: Write> Interpreter<W> {
    /// Erzeugt einen Interpreter mit leerem globalen Scope.
    pub fn new(out: W) -> Self {
        Self {
            env: Environment::new_global(),
            out,
            depth: 0,
            max_depth: DEFAULT_MAX_CALL_DEPTH,
        }
    }

    /// Setzt die maximale Aufruftiefe.
    #[must_use]
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Führt ein Programm im globalen Scope aus.
    pub fn run(&mut self, program: &[Stmt]) -> Result<()> {
        for stmt in program {
            if let Flow::Return(_) = self.exec_stmt(stmt)? {
                return Err(LumenError::runtime(
                    "'return' außerhalb einer Funktion",
                    stmt.span,
                ));
            }
        }
        Ok(())
    }

    // ---- Anweisungen ------------------------------------------------------

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow> {
        match &stmt.kind {
            StmtKind::Expr(expr) => {
                self.eval(expr)?;
                Ok(Flow::Normal)
            }
            StmtKind::Print(expr) => {
                let value = self.eval(expr)?;
                writeln!(self.out, "{value}")?;
                Ok(Flow::Normal)
            }
            StmtKind::Let { name, init } => {
                let value = match init {
                    Some(expr) => self.eval(expr)?,
                    None => Value::Nil,
                };
                self.env.borrow_mut().define(name, value);
                Ok(Flow::Normal)
            }
            StmtKind::Block(statements) => {
                let scope = Environment::with_enclosing(Rc::clone(&self.env));
                self.exec_block(statements, scope)
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.eval(condition)?.is_truthy() {
                    self.exec_stmt(then_branch)
                } else if let Some(branch) = else_branch {
                    self.exec_stmt(branch)
                } else {
                    Ok(Flow::Normal)
                }
            }
            StmtKind::While { condition, body } => {
                while self.eval(condition)?.is_truthy() {
                    if let Flow::Return(value) = self.exec_stmt(body)? {
                        return Ok(Flow::Return(value));
                    }
                }
                Ok(Flow::Normal)
            }
            StmtKind::Function(decl) => {
                let function = Value::Function(Rc::new(Function {
                    decl: Rc::clone(decl),
                    closure: Rc::clone(&self.env),
                }));
                self.env.borrow_mut().define(&decl.name, function);
                Ok(Flow::Normal)
            }
            StmtKind::Return(expr) => {
                let value = match expr {
                    Some(expr) => self.eval(expr)?,
                    None => Value::Nil,
                };
                Ok(Flow::Return(value))
            }
        }
    }

    /// Führt Anweisungen in `scope` aus und stellt den vorherigen Scope
    /// wieder her, auch wenn ein Fehler auftritt.
    fn exec_block(&mut self, statements: &[Stmt], scope: EnvRef) -> Result<Flow> {
        let previous = std::mem::replace(&mut self.env, scope);
        let result = self.exec_statements(statements);
        self.env = previous;
        result
    }

    fn exec_statements(&mut self, statements: &[Stmt]) -> Result<Flow> {
        for stmt in statements {
            if let Flow::Return(value) = self.exec_stmt(stmt)? {
                return Ok(Flow::Return(value));
            }
        }
        Ok(Flow::Normal)
    }

    // ---- Ausdrücke --------------------------------------------------------

    fn eval(&mut self, expr: &Expr) -> Result<Value> {
        match &expr.kind {
            ExprKind::Literal(literal) => Ok(literal_value(literal)),
            ExprKind::Variable(name) => {
                let value = self.env.borrow().get(name);
                value.ok_or_else(|| {
                    LumenError::runtime(format!("Undefinierte Variable '{name}'"), expr.span)
                })
            }
            ExprKind::Assign { name, value } => {
                let value = self.eval(value)?;
                // Der Clone ist nötig: Der Wert wird gespeichert und zugleich
                // als Ergebnis des Zuweisungsausdrucks zurückgegeben.
                if self.env.borrow_mut().assign(name, value.clone()) {
                    Ok(value)
                } else {
                    Err(LumenError::runtime(
                        format!("Zuweisung an undefinierte Variable '{name}'"),
                        expr.span,
                    ))
                }
            }
            ExprKind::Unary { op, operand } => {
                let value = self.eval(operand)?;
                unary(*op, value, expr.span)
            }
            ExprKind::Logical { op, left, right } => {
                let left = self.eval(left)?;
                // Kurzschluss: Der Operand selbst ist das Ergebnis, nicht nur ein Bool.
                let short_circuits = match op {
                    LogicalOp::Or => left.is_truthy(),
                    LogicalOp::And => !left.is_truthy(),
                };
                if short_circuits {
                    Ok(left)
                } else {
                    self.eval(right)
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.eval(left)?;
                let right = self.eval(right)?;
                binary(*op, &left, &right, expr.span)
            }
            ExprKind::Call { callee, args } => {
                let callee = self.eval(callee)?;
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                self.call(&callee, values, expr.span)
            }
        }
    }

    fn call(&mut self, callee: &Value, args: Vec<Value>, span: Span) -> Result<Value> {
        let Value::Function(function) = callee else {
            return Err(LumenError::runtime(
                format!("Wert vom Typ {} ist nicht aufrufbar", callee.type_name()),
                span,
            ));
        };

        let expected = function.decl.params.len();
        if args.len() != expected {
            return Err(LumenError::runtime(
                format!(
                    "'{}' erwartet {expected} Argument(e), erhielt {}",
                    function.decl.name,
                    args.len()
                ),
                span,
            ));
        }
        if self.depth >= self.max_depth {
            return Err(LumenError::runtime(
                format!("Maximale Aufruftiefe ({}) überschritten", self.max_depth),
                span,
            ));
        }

        let scope = Environment::with_enclosing(Rc::clone(&function.closure));
        {
            let mut scope_mut = scope.borrow_mut();
            for (param, arg) in function.decl.params.iter().zip(args) {
                scope_mut.define(param, arg);
            }
        }

        self.depth += 1;
        let result = self.exec_block(&function.decl.body, scope);
        self.depth -= 1;

        match result? {
            Flow::Return(value) => Ok(value),
            Flow::Normal => Ok(Value::Nil),
        }
    }
}

// ---- Reine Hilfsfunktionen (kein Interpreter-Zustand nötig) ---------------

fn literal_value(literal: &Literal) -> Value {
    match literal {
        Literal::Int(i) => Value::Int(*i),
        Literal::Float(x) => Value::Float(*x),
        Literal::Str(s) => Value::Str(Rc::clone(s)),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Nil => Value::Nil,
    }
}

fn unary(op: UnaryOp, value: Value, span: Span) -> Result<Value> {
    match (op, value) {
        (UnaryOp::Not, value) => Ok(Value::Bool(!value.is_truthy())),
        (UnaryOp::Neg, Value::Int(i)) => i
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| LumenError::runtime("Ganzzahl-Überlauf", span)),
        (UnaryOp::Neg, Value::Float(x)) => Ok(Value::Float(-x)),
        (UnaryOp::Neg, other) => Err(LumenError::runtime(
            format!("Unäres '-' erwartet eine Zahl, erhielt {}", other.type_name()),
            span,
        )),
    }
}

fn binary(op: BinaryOp, left: &Value, right: &Value, span: Span) -> Result<Value> {
    match (op, left, right) {
        (BinaryOp::Eq, _, _) => Ok(Value::Bool(left == right)),
        (BinaryOp::NotEq, _, _) => Ok(Value::Bool(left != right)),
        (BinaryOp::Add, Value::Str(a), Value::Str(b)) => {
            let mut joined = String::with_capacity(a.len() + b.len());
            joined.push_str(a);
            joined.push_str(b);
            Ok(Value::Str(Rc::from(joined)))
        }
        _ => numeric(op, left, right, span),
    }
}

/// Arithmetik und Vergleiche auf Zahlen. Gemischte Operanden werden zu Float.
fn numeric(op: BinaryOp, left: &Value, right: &Value, span: Span) -> Result<Value> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => int_op(op, *a, *b, span),
        (Value::Int(a), Value::Float(b)) => float_op(op, *a as f64, *b, span),
        (Value::Float(a), Value::Int(b)) => float_op(op, *a, *b as f64, span),
        (Value::Float(a), Value::Float(b)) => float_op(op, *a, *b, span),
        _ => Err(LumenError::runtime(
            format!(
                "Operator '{}' ist für {} und {} nicht definiert",
                op.symbol(),
                left.type_name(),
                right.type_name()
            ),
            span,
        )),
    }
}

fn int_op(op: BinaryOp, a: i64, b: i64, span: Span) -> Result<Value> {
    let overflow = || LumenError::runtime("Ganzzahl-Überlauf", span);
    let div_zero = || LumenError::runtime("Division durch Null", span);
    Ok(match op {
        BinaryOp::Add => Value::Int(a.checked_add(b).ok_or_else(overflow)?),
        BinaryOp::Sub => Value::Int(a.checked_sub(b).ok_or_else(overflow)?),
        BinaryOp::Mul => Value::Int(a.checked_mul(b).ok_or_else(overflow)?),
        BinaryOp::Div => {
            if b == 0 {
                return Err(div_zero());
            }
            Value::Int(a.checked_div(b).ok_or_else(overflow)?)
        }
        BinaryOp::Rem => {
            if b == 0 {
                return Err(div_zero());
            }
            Value::Int(a.checked_rem(b).ok_or_else(overflow)?)
        }
        BinaryOp::Eq => Value::Bool(a == b),
        BinaryOp::NotEq => Value::Bool(a != b),
        BinaryOp::Lt => Value::Bool(a < b),
        BinaryOp::LtEq => Value::Bool(a <= b),
        BinaryOp::Gt => Value::Bool(a > b),
        BinaryOp::GtEq => Value::Bool(a >= b),
    })
}

fn float_op(op: BinaryOp, a: f64, b: f64, span: Span) -> Result<Value> {
    let div_zero = || LumenError::runtime("Division durch Null", span);
    Ok(match op {
        BinaryOp::Add => Value::Float(a + b),
        BinaryOp::Sub => Value::Float(a - b),
        BinaryOp::Mul => Value::Float(a * b),
        BinaryOp::Div => {
            if b == 0.0 {
                return Err(div_zero());
            }
            Value::Float(a / b)
        }
        BinaryOp::Rem => {
            if b == 0.0 {
                return Err(div_zero());
            }
            Value::Float(a % b)
        }
        BinaryOp::Eq => Value::Bool(a == b),
        BinaryOp::NotEq => Value::Bool(a != b),
        BinaryOp::Lt => Value::Bool(a < b),
        BinaryOp::LtEq => Value::Bool(a <= b),
        BinaryOp::Gt => Value::Bool(a > b),
        BinaryOp::GtEq => Value::Bool(a >= b),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_source, run_to_string};

    fn out(source: &str) -> String {
        run_to_string(source).expect("Programm sollte laufen")
    }

    fn runtime_error(source: &str) -> String {
        match run_to_string(source) {
            Err(LumenError::Runtime { message, .. }) => message,
            other => panic!("Laufzeitfehler erwartet, erhielt {other:?}"),
        }
    }

    #[test]
    fn arithmetic_respects_precedence() {
        assert_eq!(out("print 2 + 3 * 4;"), "14\n");
        assert_eq!(out("print (2 + 3) * 4;"), "20\n");
        assert_eq!(out("print -2 + 5;"), "3\n");
        assert_eq!(out("print 10 % 4;"), "2\n");
    }

    #[test]
    fn integer_and_float_division() {
        assert_eq!(out("print 7 / 2;"), "3\n");
        assert_eq!(out("print 7.0 / 2;"), "3.5\n");
    }

    #[test]
    fn mixed_arithmetic_promotes_to_float() {
        assert_eq!(out("print 1 + 2.5;"), "3.5\n");
        assert_eq!(out("print 1.5 + 1.5;"), "3.0\n");
    }

    #[test]
    fn string_concatenation() {
        assert_eq!(out(r#"print "ab" + "cd";"#), "abcd\n");
    }

    #[test]
    fn comparisons_and_equality() {
        assert_eq!(out("print 1 < 2;"), "true\n");
        assert_eq!(out("print 2 <= 1;"), "false\n");
        assert_eq!(out("print 1 == 1.0;"), "true\n");
        assert_eq!(out(r#"print "a" == "a";"#), "true\n");
        assert_eq!(out("print nil == false;"), "false\n");
        assert_eq!(out("print 1 != 2;"), "true\n");
    }

    #[test]
    fn logical_operators_return_operands() {
        assert_eq!(out("print nil || 5;"), "5\n");
        assert_eq!(out("print 1 && 2;"), "2\n");
        assert_eq!(out("print !nil;"), "true\n");
        assert_eq!(out("print !0;"), "false\n");
    }

    #[test]
    fn logical_operators_short_circuit() {
        let source = "
            let x = 0;
            fn bump() { x = x + 1; return true; }
            false && bump();
            true || bump();
            print x;
        ";
        assert_eq!(out(source), "0\n");
    }

    #[test]
    fn variables_and_assignment() {
        assert_eq!(out("let a = 1; a = a + 1; print a;"), "2\n");
        assert_eq!(out("let a; print a;"), "nil\n");
        assert_eq!(out("let a = 1; let b = a = 5; print b; print a;"), "5\n5\n");
    }

    #[test]
    fn blocks_create_scopes() {
        assert_eq!(out("let a = 1; { let a = 2; print a; } print a;"), "2\n1\n");
        assert_eq!(out("let a = 1; { a = 5; } print a;"), "5\n");
    }

    #[test]
    fn if_else_chain() {
        let source = "
            let x = 5;
            if x < 3 { print 1; } else if x < 10 { print 2; } else { print 3; }
        ";
        assert_eq!(out(source), "2\n");
    }

    #[test]
    fn while_loop() {
        let source = "
            let i = 0; let sum = 0;
            while i < 5 { sum = sum + i; i = i + 1; }
            print sum;
        ";
        assert_eq!(out(source), "10\n");
    }

    #[test]
    fn recursion_computes_fibonacci() {
        let source = "
            fn fib(n) {
                if n < 2 { return n; }
                return fib(n - 1) + fib(n - 2);
            }
            print fib(10);
        ";
        assert_eq!(out(source), "55\n");
    }

    #[test]
    fn closures_capture_their_environment() {
        let source = "
            fn make_counter() {
                let n = 0;
                fn inc() { n = n + 1; return n; }
                return inc;
            }
            let a = make_counter();
            let b = make_counter();
            print a(); print a(); print b();
        ";
        assert_eq!(out(source), "1\n2\n1\n");
    }

    #[test]
    fn function_without_return_yields_nil() {
        assert_eq!(out("fn f() {} print f();"), "nil\n");
        assert_eq!(out("fn f() { return; } print f();"), "nil\n");
    }

    #[test]
    fn return_exits_loops_early() {
        let source = "
            fn first_over(n) {
                let i = 0;
                while true { if i > n { return i; } i = i + 1; }
            }
            print first_over(3);
        ";
        assert_eq!(out(source), "4\n");
    }

    #[test]
    fn functions_print_by_name() {
        assert_eq!(out("fn f() {} print f;"), "<fn f>\n");
    }

    #[test]
    fn undefined_variable() {
        assert!(runtime_error("print y;").contains("Undefinierte Variable 'y'"));
        assert!(runtime_error("y = 1;").contains("undefinierte Variable"));
    }

    #[test]
    fn type_errors() {
        assert!(runtime_error(r#"print 1 + "a";"#).contains("int und string"));
        assert!(runtime_error("print -true;").contains("Zahl"));
        assert!(runtime_error(r#"print "a" < "b";"#).contains("'<'"));
    }

    #[test]
    fn division_by_zero() {
        assert!(runtime_error("print 1 / 0;").contains("Division durch Null"));
        assert!(runtime_error("print 1.0 / 0;").contains("Division durch Null"));
        assert!(runtime_error("print 1 % 0;").contains("Division durch Null"));
    }

    #[test]
    fn integer_overflow() {
        assert!(runtime_error("print 9223372036854775807 + 1;").contains("Überlauf"));
        assert!(runtime_error("print 9223372036854775807 * 2;").contains("Überlauf"));
    }

    #[test]
    fn wrong_argument_count() {
        let message = runtime_error("fn f(a, b) { return a; } f(1);");
        assert!(message.contains("erwartet 2"));
        assert!(message.contains("erhielt 1"));
    }

    #[test]
    fn calling_a_non_function() {
        assert!(runtime_error("let x = 1; x();").contains("nicht aufrufbar"));
    }

    #[test]
    fn return_outside_function() {
        assert!(runtime_error("return 1;").contains("außerhalb"));
    }

    #[test]
    fn runaway_recursion_is_stopped() {
        let program = parse_source("fn f() { return f(); } f();").expect("sollte parsen");
        let mut buffer = Vec::new();
        let mut interpreter = Interpreter::new(&mut buffer).with_max_depth(50);
        match interpreter.run(&program) {
            Err(LumenError::Runtime { message, .. }) => {
                assert!(message.contains("Aufruftiefe"));
            }
            other => panic!("Laufzeitfehler erwartet, erhielt {other:?}"),
        }
    }

    #[test]
    fn scope_is_restored_after_a_runtime_error() {
        let program = parse_source("{ let a = 1; print b; }").expect("sollte parsen");
        let mut buffer = Vec::new();
        let mut interpreter = Interpreter::new(&mut buffer);
        assert!(interpreter.run(&program).is_err());
        // Nach dem Fehler muss der globale Scope wieder aktiv sein: `a` war nur im Block sichtbar.
        let follow_up = parse_source("print a;").expect("sollte parsen");
        assert!(interpreter.run(&follow_up).is_err());
    }
}

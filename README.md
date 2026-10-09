# Lumen

Lumen ist eine kleine, dynamisch typisierte Programmiersprache mit Tree-Walking-Interpreter, geschrieben in Rust. Das Projekt zeigt die klassische Pipeline einer Sprache in überschaubarem Umfang: Lexer, Parser, AST und Auswertung.

```
fn fib(n) {
    if n < 2 { return n; }
    return fib(n - 1) + fib(n - 2);
}
print fib(10);   // 55
```

## Features

- **Datentypen:** `int` (i64), `float` (f64), `bool`, `string`, `nil`
- **Variablen und Scopes:** `let`, Zuweisung mit `=`, Blockscoping
- **Kontrollfluss:** `if` / `else if` / `else`, `while`
- **Funktionen:** `fn`, `return`, Rekursion und Closures
- **Operatoren:** Arithmetik, Vergleiche, `&&` / `||` (mit Kurzschluss), `!`
- **Fehlermeldungen mit Quellposition:** Zeile, Spalte und markierte Quelltextzeile

```
Laufzeitfehler bei 2:7: Undefinierte Variable 'y'
  |
2 | print y;
  |       ^
```

## Schnellstart

Voraussetzung ist Rust ab Version 1.74.

```bash
cargo run -- examples/beispiel.lm   # Programm ausführen
cargo test                          # Tests ausführen
cargo clippy --all-targets          # Lints prüfen
```

Lumen lässt sich auch als Bibliothek einbinden:

```rust
let output = lumen::run_to_string("print 1 + 2;").unwrap();
assert_eq!(output, "3\n");
```

## Architektur

```
Quelltext → Lexer → Tokens → Parser → AST → Interpreter
```

| Datei | Aufgabe |
|---|---|
| `token.rs` | Tokens und `Span` (Position im Quelltext) |
| `lexer.rs` | Quelltext zu Tokens |
| `ast.rs` | Syntaxbaum, unabhängig von der Auswertung |
| `parser.rs` | Rekursiver Abstieg für Anweisungen, Pratt-Parser für Ausdrücke |
| `value.rs` | Laufzeitwerte |
| `environment.rs` | Verkettete Scopes (`Rc<RefCell<..>>`) |
| `interpreter.rs` | Tree-Walking-Auswertung des AST |
| `error.rs` | Fehlertypen (`thiserror`) und Fehlerdarstellung |

Der AST kennt keine Auswertung. Dadurch kann später ein Bytecode-Compiler denselben Baum nutzen, ohne dass Lexer und Parser angepasst werden müssen.

## Design-Entscheidungen

- **`Rc<RefCell<Environment>>`:** Closures brauchen geteilten Besitz an Scopes. Eine Funktion, die ihren eigenen Scope einfängt, bildet dabei einen Referenzzyklus. Eine Arena oder ein Garbage Collector würde das beheben.
- **`return` als Kontrollfluss:** Der Interpreter unterscheidet zwischen normalem Ende und `return`, statt dafür Fehler zu missbrauchen.
- **Aufruftiefe begrenzt:** Unendliche Rekursion ergibt einen Laufzeitfehler statt eines Stack-Überlaufs.
- **Keine Panics bei Nutzereingaben:** Ganzzahl-Überlauf und Division durch Null sind Laufzeitfehler.
- **Testbare Ausgabe:** `print` schreibt in ein injizierbares `Write`, sodass Tests die Ausgabe prüfen können.

## Roadmap

- [ ] Eingebautes Tracing und Profiling auf Sprachebene
- [ ] Compiler vom AST zu Bytecode und Stack-basierte VM
- [ ] Eingebaute Funktionen (z. B. `clock`, `len`)
- [ ] Fehlerwiederherstellung im Parser (mehrere Fehler pro Lauf)
- [ ] REPL

## Lizenz

MIT

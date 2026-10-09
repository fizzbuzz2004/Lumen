//! Kommandozeile: `lumen <datei.lm>`

use std::process::ExitCode;
use std::{env, fs, io, thread};

use lumen::run_source;

/// Der Interpreter ist rekursiv; ein großer Stack erlaubt tiefe Lumen-Rekursion.
const STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("Verwendung: lumen <datei.lm>");
        return ExitCode::from(64);
    };

    let worker = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || execute(&path));

    match worker.map(thread::JoinHandle::join) {
        Ok(Ok(code)) => code,
        Ok(Err(_)) => {
            eprintln!("Interner Fehler: Interpreter-Thread ist abgestürzt");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("Interpreter-Thread konnte nicht gestartet werden: {err}");
            ExitCode::FAILURE
        }
    }
}

fn execute(path: &str) -> ExitCode {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("Kann '{path}' nicht lesen: {err}");
            return ExitCode::from(66);
        }
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    match run_source(&source, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.render(&source));
            ExitCode::from(70)
        }
    }
}

//! End-to-End-Tests über die öffentliche API.

use lumen::{run_to_string, LumenError};

#[test]
fn example_program_prints_fib_10() {
    let source = include_str!("../examples/example.lm");
    assert_eq!(run_to_string(source).expect("Beispiel sollte laufen"), "55\n");
}

#[test]
fn comments_and_whitespace_are_ignored() {
    let source = "// Kommentar\n\n   print 1; // noch einer\n";
    assert_eq!(run_to_string(source).expect("sollte laufen"), "1\n");
}

#[test]
fn larger_program_mixes_features() {
    let source = r#"
        fn is_even(n) { return n % 2 == 0; }
        let i = 1;
        let evens = "";
        while i <= 6 {
            if is_even(i) { evens = evens + "x"; }
            i = i + 1;
        }
        print evens;
    "#;
    assert_eq!(run_to_string(source).expect("sollte laufen"), "xxx\n");
}

#[test]
fn lexer_errors_surface_through_the_public_api() {
    assert!(matches!(
        run_to_string("print @;"),
        Err(LumenError::Lex { .. })
    ));
}

#[test]
fn parse_errors_surface_through_the_public_api() {
    assert!(matches!(
        run_to_string("print ;"),
        Err(LumenError::Parse { .. })
    ));
}

#[test]
fn runtime_errors_carry_line_and_column() {
    let err = run_to_string("let x = 1;\nprint y;").unwrap_err();
    match err {
        LumenError::Runtime { span, .. } => assert_eq!((span.line, span.column), (2, 7)),
        other => panic!("Laufzeitfehler erwartet, erhielt {other:?}"),
    }
}

#[test]
fn rendered_errors_show_the_source_line() {
    let source = "let x = 1;\nprint y;";
    let rendered = run_to_string(source).unwrap_err().render(source);
    assert!(rendered.contains("2 | print y;"));
    assert!(rendered.contains('^'));
}

#[test]
fn output_before_a_runtime_error_is_not_lost_in_the_buffer() {
    let mut buffer = Vec::new();
    let result = lumen::run_source("print 1; print y;", &mut buffer);
    assert!(result.is_err());
    assert_eq!(String::from_utf8_lossy(&buffer), "1\n");
}

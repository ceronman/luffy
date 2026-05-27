pub mod ast;

use crate::error::{CompilerError, ErrorKind};
use crate::lexer::Span;
use ariadne::{Config, Label, Report, ReportKind, Source};

impl ariadne::Span for Span {
    type SourceId = ();

    fn source(&self) -> &Self::SourceId {
        &()
    }
    fn start(&self) -> usize {
        self.start
    }
    fn end(&self) -> usize {
        self.end
    }
}

pub fn print_error(src: &str, error: &CompilerError) {
    Report::build(ReportKind::Error, error.span)
        .with_config(Config::default().with_compact(false).with_underlines(true))
        .with_message("Compiler Error")
        .with_label(Label::new(error.span).with_message(&error.msg))
        .finish()
        .eprint(Source::from(src))
        .unwrap();
}

pub fn error(src: &str, error: &CompilerError) -> String {
    let mut out: Vec<u8> = Vec::new();
    Report::build(ReportKind::Error, error.span)
        .with_config(Config::default().with_compact(true).with_color(false))
        .with_message(match error.kind {
            ErrorKind::Parse => "Parsing failed",
            ErrorKind::Resolve => "Name resolution failed",
            ErrorKind::Type => "Type ",
            ErrorKind::Compile => "Compile error",
        })
        .with_label(Label::new(error.span).with_message(&error.msg))
        .finish()
        .write(Source::from(src), &mut out)
        .unwrap();
    String::from_utf8(out).unwrap()
}

pub fn annotate_error(src: &str, error: &CompilerError) -> String {
    let mut result = String::new();
    let mut offset = 0;
    let mut annotated = false;
    for line in src.split_inclusive('\n') {
        result.push_str(line);
        if !annotated && offset + line.len() > error.span.start {
            let start = error.span.start - offset;
            let start = start.saturating_sub(2);
            let len = error.span.end - error.span.start;
            let annotation = format!("{}//{} {}\n", " ".repeat(start), "^".repeat(len), error.msg);
            result.push_str(&annotation);
            annotated = true
        }
        offset += line.len();
    }
    if !annotated {
        result.push_str(&format!("\n// {}", error.msg));
    }
    result
}

pub fn annotate_error_single(src: &str, error: &CompilerError) -> String {
    let mut result = String::new();
    let mut offset = 0;
    let mut annotated = false;
    for line in src.split_inclusive('\n') {
        if !annotated && offset + line.len() > error.span.start {
            let start = error.span.start - offset;
            let len = error.span.end - error.span.start;
            let annotation = format!(
                "{}{} ─── {}\n",
                " ".repeat(start),
                "^".repeat(len),
                error.msg
            );
            result.push_str(line);
            if !line.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(&annotation);
            annotated = true
        }
        offset += line.len();
    }
    if !annotated {
        result.push_str(&format!("\n// {}", error.msg));
    }
    result
}

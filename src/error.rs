use crate::source::Span;
use std::fmt::Debug;

#[derive(Debug)]
pub struct CompilerError {
    pub kind: ErrorKind,
    pub msg: String,
    pub span: Span,
}

#[derive(Debug)]
pub enum ErrorKind {
    Parse,
    Resolve,
    Type,
    Compile,
    Internal,
}

pub fn parse_err<T>(span: Span, message: impl Into<String>) -> crate::parser::Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Parse,
        msg: message.into(),
        span,
    })
}

pub fn resolve_err<T>(span: Span, message: impl Into<String>) -> crate::parser::Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Resolve,
        msg: message.into(),
        span,
    })
}

pub fn type_err<T>(span: Span, message: impl Into<String>) -> crate::parser::Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Type,
        msg: message.into(),
        span,
    })
}

pub fn internal_err<T>(span: Span, message: impl Into<String>) -> crate::parser::Result<T> {
    Err(CompilerError {
        kind: ErrorKind::Internal,
        msg: message.into(),
        span,
    })
}

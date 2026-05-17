use crate::lexer::Span;

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
}

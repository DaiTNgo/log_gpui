#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub message: String,
    pub span: std::ops::Range<usize>,
    pub severity: DiagnosticSeverity,
}

impl Diagnostic {
    pub fn error(message: String, span: std::ops::Range<usize>) -> Self {
        Self {
            message,
            span,
            severity: DiagnosticSeverity::Error,
        }
    }
}

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub path: PathBuf,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn new(
        code: DiagnosticCode,
        severity: Severity,
        path: PathBuf,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            path,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    MissingContentsJson,
    InvalidContentsJson,
    InvalidContentsSchema,
    MissingReferencedFile,
    UnsupportedFolderType,
    UnreadableDirectory,
    UnreadableFile,
}

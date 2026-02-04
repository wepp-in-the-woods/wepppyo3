use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Duplicate,
    Exists,
    NotInManifest,
    Missing,
    HeaderError,
    ParseError,
    ColumnMismatch,
    DecodeError,
    FileChanged,
    PathInvalid,
    ManifestMissing,
    ManifestUnreadable,
    InterchangeCreatedUtcInvalid,
    InterchangeInProgress,
    InterchangeComplete,
    InterchangePartial,
    InterchangeFailed,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Duplicate => "duplicate",
            Reason::Exists => "exists",
            Reason::NotInManifest => "not_in_manifest",
            Reason::Missing => "missing",
            Reason::HeaderError => "header_error",
            Reason::ParseError => "parse_error",
            Reason::ColumnMismatch => "column_mismatch",
            Reason::DecodeError => "decode_error",
            Reason::FileChanged => "file_changed",
            Reason::PathInvalid => "path_invalid",
            Reason::ManifestMissing => "manifest_missing",
            Reason::ManifestUnreadable => "manifest_unreadable",
            Reason::InterchangeCreatedUtcInvalid => "interchange_created_utc_invalid",
            Reason::InterchangeInProgress => "interchange_in_progress",
            Reason::InterchangeComplete => "interchange_complete",
            Reason::InterchangePartial => "interchange_partial",
            Reason::InterchangeFailed => "interchange_failed",
        }
    }

    pub fn is_error_class(self) -> bool {
        matches!(
            self,
            Reason::HeaderError
                | Reason::ParseError
                | Reason::ColumnMismatch
                | Reason::DecodeError
                | Reason::FileChanged
        )
    }
}

#[derive(Error, Debug)]
pub enum SwatError {
    #[error("I/O error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Parse error at {path:?}: {message}")]
    Parse {
        path: PathBuf,
        line: Option<usize>,
        message: String,
        preview: Option<String>,
        reason: Reason,
    },
    #[error("Decode error at {path:?}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("Value error: {message}")]
    Value { message: String },
    #[error("Arrow error: {0}")]
    Arrow(String),
    #[error("Parquet error: {0}")]
    Parquet(String),
}

impl SwatError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn parse(
        path: impl Into<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
        preview: Option<String>,
    ) -> Self {
        Self::Parse {
            path: path.into(),
            line,
            message: message.into(),
            preview,
            reason: Reason::ParseError,
        }
    }

    pub fn header_error(
        path: impl Into<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
        preview: Option<String>,
    ) -> Self {
        Self::Parse {
            path: path.into(),
            line,
            message: message.into(),
            preview,
            reason: Reason::HeaderError,
        }
    }

    pub fn column_mismatch(
        path: impl Into<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
        preview: Option<String>,
    ) -> Self {
        Self::Parse {
            path: path.into(),
            line,
            message: message.into(),
            preview,
            reason: Reason::ColumnMismatch,
        }
    }

    pub fn file_changed(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Parse {
            path: path.into(),
            line: None,
            message: message.into(),
            preview: None,
            reason: Reason::FileChanged,
        }
    }

    pub fn decode(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Decode {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn value(message: impl Into<String>) -> Self {
        Self::Value {
            message: message.into(),
        }
    }

    pub fn reason(&self) -> Reason {
        match self {
            SwatError::Parse { reason, .. } => *reason,
            SwatError::Decode { .. } => Reason::DecodeError,
            SwatError::Io { .. } | SwatError::Arrow(_) | SwatError::Parquet(_) => {
                Reason::ParseError
            }
            SwatError::Value { .. } => Reason::ParseError,
        }
    }

    pub fn display_message(&self) -> String {
        match self {
            SwatError::Parse {
                path,
                line,
                message,
                preview,
                ..
            } => {
                let line_info = line.map(|line| format!(":{line}")).unwrap_or_default();
                let preview_info = preview
                    .as_ref()
                    .map(|preview| format!(" (line: {preview})"))
                    .unwrap_or_default();
                format!("{}{}: {message}{}", path.display(), line_info, preview_info)
            }
            _ => self.to_string(),
        }
    }
}

impl From<arrow2::error::Error> for SwatError {
    fn from(value: arrow2::error::Error) -> Self {
        SwatError::Arrow(value.to_string())
    }
}

impl From<parquet2::error::Error> for SwatError {
    fn from(value: parquet2::error::Error) -> Self {
        SwatError::Parquet(value.to_string())
    }
}

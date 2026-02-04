use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum InterchangeError {
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
    },
    #[error("Calendar error: {message}")]
    Calendar { message: String },
    #[error("Arrow error: {0}")]
    Arrow(String),
    #[error("Parquet error: {0}")]
    Parquet(String),
}

impl InterchangeError {
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
        }
    }

    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    fn line_info(&self) -> String {
        match self {
            Self::Parse {
                line: Some(line), ..
            } => format!(":{line}"),
            _ => String::new(),
        }
    }

    fn preview_info(&self) -> String {
        match self {
            Self::Parse {
                preview: Some(preview),
                ..
            } => format!(" (line: {preview})"),
            _ => String::new(),
        }
    }

    pub fn display_message(&self) -> String {
        match self {
            Self::Parse { path, message, .. } => {
                format!(
                    "{}{}: {message}{}",
                    path.display(),
                    self.line_info(),
                    self.preview_info()
                )
            }
            _ => self.to_string(),
        }
    }
}

impl From<arrow2::error::Error> for InterchangeError {
    fn from(value: arrow2::error::Error) -> Self {
        Self::Arrow(value.to_string())
    }
}

impl From<parquet2::error::Error> for InterchangeError {
    fn from(value: parquet2::error::Error) -> Self {
        Self::Parquet(value.to_string())
    }
}

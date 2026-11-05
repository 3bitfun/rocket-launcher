use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize)]
#[error("{message}")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn io(action: &str, error: impl std::fmt::Display) -> Self {
        Self::new("FILESYSTEM", format!("Could not {action}: {error}"))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("TOML error at {path}: {source}")]
    Toml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("{0}")]
    Command(CommandError),
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub exit_code: i32,
    pub details: serde_json::Value,
    pub json: bool,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CommandError {}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            exit_code: 1,
            details: serde_json::json!({}),
            json: false,
        }
    }

    pub fn exit_code(mut self, exit_code: i32) -> Self {
        self.exit_code = exit_code;
        self
    }

    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    pub fn json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "error": {
                "code": self.code,
                "message": self.message,
                "details": self.details,
            }
        })
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

pub fn message<T>(message: impl Into<String>) -> Result<T> {
    Err(AppError::Message(message.into()))
}


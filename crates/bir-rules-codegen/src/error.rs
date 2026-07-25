use std::error::Error as StdError;
use std::fmt;
use std::path::Path;

pub type Result<T> = std::result::Result<T, CodegenError>;

#[derive(Debug)]
pub struct CodegenError {
    message: String,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl CodegenError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        message: impl Into<String>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub fn io(action: &str, path: &Path, source: std::io::Error) -> Self {
        Self::with_source(format!("{action} `{}` failed", path.display()), source)
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CodegenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for CodegenError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}

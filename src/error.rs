
#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("usage: {0}")]
    Usage(String),

    #[error("input error: {0}")]
    Input(String),

    #[error("model not installed: {0}")]
    ModelNotInstalled(String),

    #[error("unsupported language pair: {0}")]
    UnsupportedPair(String),

    #[error("translation failure: {0}")]
    TranslationFailure(String),

    #[error("network guard violation: {0}")]
    NetworkGuard(String),

    #[error("server error: {0}")]
    Server(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl TranslateError {
    pub fn exit_code(&self) -> i32 {
        match self {
            TranslateError::Usage(_) => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TranslateError {
    #[error("translate: {0}")]
    Usage(String),

    #[error("translate: {0}")]
    Input(String),

    #[error("translate: translation failed: {0}")]
    TranslationFailure(String),

    #[error("translate: io error: {0}")]
    Io(String),

    #[error("translate: Apple Translation requires macOS 26 Tahoe or newer on Apple silicon")]
    UnsupportedOS,

    #[error("translate: translation model '{0}' is not installed (run `translate --install {0}`)")]
    ModelNotInstalled(String),

    #[error("translate: language pair '{0}' is not supported by Apple Translation (run `translate --available`)")]
    UnsupportedPair(String),

    #[error("translate: network guard violation: {0}")]
    NetworkGuard(String),

    #[error("translate: server error: {0}")]
    Server(String),
}

impl TranslateError {
    pub fn exit_code(&self) -> i32 {
        match self {
            TranslateError::Usage(_) => 1,
            TranslateError::Input(_) => 1,
            TranslateError::TranslationFailure(_) => 2,
            TranslateError::Io(_) => 2,
            TranslateError::UnsupportedOS => 3,
            TranslateError::ModelNotInstalled(_) => 4,
            TranslateError::UnsupportedPair(_) => 5,
            TranslateError::NetworkGuard(_) => 1,
            TranslateError::Server(_) => 1,
        }
    }
}

impl From<std::io::Error> for TranslateError {
    fn from(err: std::io::Error) -> Self {
        TranslateError::Io(err.to_string())
    }
}

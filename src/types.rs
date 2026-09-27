use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Plain,
    Json,
    Ndjson,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Plain
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PairStatus {
    Installed,
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub language_code: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationRecord {
    pub from: String,
    pub to: String,
    pub src: String,
    pub dst: String,
    pub conf: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AugeResponse<T> {
    pub mode: String,
    pub file: Option<String>,
    pub results: T,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub on_device: bool,
    pub version: String,
    pub schema: String,
}

/// Catalog of standard 33 languages supported by Apple on-device translation
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("ar", "Arabic"),
    ("bg", "Bulgarian"),
    ("cs", "Czech"),
    ("da", "Danish"),
    ("de", "German"),
    ("el", "Greek"),
    ("en", "English"),
    ("es", "Spanish"),
    ("et", "Estonian"),
    ("fi", "Finnish"),
    ("fr", "French"),
    ("hi", "Hindi"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("nb", "Norwegian"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("pt", "Portuguese"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sk", "Slovak"),
    ("sl", "Slovenian"),
    ("sv", "Swedish"),
    ("th", "Thai"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("vi", "Vietnamese"),
    ("zh", "Chinese"),
];

/// Normalize language codes (e.g., "en-US" -> "en", "zh-Hans" -> "zh", "DE" -> "de")
pub fn normalize_language_code(code: &str) -> String {
    let lower = code.trim().to_lowercase();
    if let Some((prefix, _)) = lower.split_once('-') {
        prefix.to_string()
    } else if let Some((prefix, _)) = lower.split_once('_') {
        prefix.to_string()
    } else {
        lower
    }
}

pub fn is_language_supported(code: &str) -> bool {
    let normalized = normalize_language_code(code);
    SUPPORTED_LANGUAGES.iter().any(|(c, _)| *c == normalized)
}

pub fn parse_language_hints(langs: Option<&str>) -> Vec<String> {
    match langs {
        Some(s) => s
            .split(',')
            .map(|item| normalize_language_code(item.trim()))
            .filter(|item| !item.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

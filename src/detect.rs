use crate::error::TranslateError;
use crate::types::{is_language_supported, normalize_language_code, DetectionResult};
use std::path::PathBuf;
use std::process::Command;
use whatlang::detect;

pub fn iso639_3_to_639_1(code: &str) -> &str {
    match code {
        "eng" => "en",
        "spa" => "es",
        "deu" | "ger" => "de",
        "fra" | "fre" => "fr",
        "ita" => "it",
        "por" => "pt",
        "rus" => "ru",
        "cmn" | "zho" | "chi" => "zh",
        "jpn" => "ja",
        "kor" => "ko",
        "ara" => "ar",
        "hin" => "hi",
        "nld" | "dut" => "nl",
        "pol" => "pl",
        "tur" => "tr",
        "vie" => "vi",
        "ukr" => "uk",
        "tha" => "th",
        "swe" => "sv",
        "dan" => "da",
        "fin" => "fi",
        "ell" | "gre" => "el",
        "ces" | "cze" => "cs",
        "hun" => "hu",
        "ind" => "id",
        "nor" | "nob" => "nb",
        "slk" | "slo" => "sk",
        "slv" => "sl",
        "bul" => "bg",
        "est" => "et",
        "lav" => "lv",
        "lit" => "lt",
        "ron" | "rum" => "ro",
        other => other,
    }
}

#[derive(Debug, Clone, Default)]
pub struct LanguageDetector {
    hints: Vec<String>,
}

impl LanguageDetector {
    pub fn new(hints: &[String]) -> Self {
        Self {
            hints: hints.iter().map(|h| normalize_language_code(h)).collect(),
        }
    }

    pub fn detect(&self, text: &str) -> Result<DetectionResult, TranslateError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(TranslateError::Input("text is empty; cannot detect language".into()));
        }

        // 1. Try Apple's on-device NaturalLanguage framework first
        if let Some(res) = Self::detect_with_apple_bridge(trimmed) {
            if is_language_supported(&res.language_code) {
                return Ok(res);
            }
        }

        // 2. High-speed whatlang fallback
        if let Some(info) = detect(trimmed) {
            let code_3 = info.lang().code();
            let mut code = iso639_3_to_639_1(code_3).to_string();
            let mut conf = info.confidence();

            // If hints are provided and matched, boost confidence
            if !self.hints.is_empty() {
                if self.hints.contains(&code) {
                    conf = (conf * 1.2).min(1.0);
                } else if let Some(first_hint) = self.hints.first() {
                    if conf < 0.3 {
                        code = first_hint.clone();
                    }
                }
            }

            if is_language_supported(&code) && conf >= 0.2 {
                return Ok(DetectionResult {
                    language_code: code,
                    confidence: (conf * 100.0).round() / 100.0,
                });
            }
        }

        // Default to English if ambiguous
        Ok(DetectionResult {
            language_code: "en".into(),
            confidence: 0.5,
        })
    }

    fn detect_with_apple_bridge(text: &str) -> Option<DetectionResult> {
        let mut bridge_path = None;

        if let Some(bin_str) = option_env!("APPLE_TRANSLATE_BRIDGE_BIN") {
            let p = PathBuf::from(bin_str);
            if p.exists() {
                bridge_path = Some(p);
            }
        }

        if bridge_path.is_none() {
            let p = PathBuf::from("bridge/apple_translate_bridge");
            if p.exists() {
                bridge_path = Some(p);
            }
        }

        if bridge_path.is_none() {
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    let p = dir.join("apple_translate_bridge");
                    if p.exists() {
                        bridge_path = Some(p);
                    }
                }
            }
        }

        let p = bridge_path?;
        let output = Command::new(p)
            .args(["detect", text])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut parts = stdout.trim().split('\t');
        let code = normalize_language_code(parts.next()?);
        let conf: f64 = parts.next()?.parse().ok()?;

        Some(DetectionResult {
            language_code: code,
            confidence: (conf * 100.0).round() / 100.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_english() {
        let detector = LanguageDetector::new(&[]);
        let result = detector.detect("This is a simple sentence written in the English language.").unwrap();
        assert_eq!(result.language_code, "en");
        assert!(result.confidence > 0.4);
    }

    #[test]
    fn test_detect_spanish() {
        let detector = LanguageDetector::new(&[]);
        let result = detector.detect("Buenos días, ¿cómo estás hoy? Este es un texto en español.").unwrap();
        assert_eq!(result.language_code, "es");
        assert!(result.confidence > 0.4);
    }

    #[test]
    fn test_detect_german() {
        let detector = LanguageDetector::new(&[]);
        let result = detector.detect("Guten Morgen, wie geht es Ihnen heute? Ich wünsche Ihnen einen schönen Tag.").unwrap();
        assert_eq!(result.language_code, "de");
        assert!(result.confidence > 0.4);
    }
}

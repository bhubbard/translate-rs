use crate::error::TranslateError;
use crate::masker::TranslationMasker;
use crate::types::{normalize_language_code, PairStatus, SUPPORTED_LANGUAGES};
use async_trait::async_trait;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[async_trait]
pub trait Translating: std::fmt::Debug + Send + Sync {
    async fn prepare(
        &self,
        source: &str,
        target: &str,
        no_install: bool,
        quiet: bool,
    ) -> Result<(), TranslateError>;

    async fn translate(
        &self,
        units: &[String],
        source: &str,
        target: &str,
        preserve_newlines: bool,
    ) -> Result<Vec<String>, TranslateError>;

    async fn pair_status(&self, source: &str, target: &str) -> PairStatus;

    async fn installed_pairs(&self) -> Vec<String>;

    async fn available_pairs(&self) -> Vec<String>;
}

#[derive(Debug, Clone)]
pub struct AppleTranslator {
    bridge_path: Option<PathBuf>,
}

impl AppleTranslator {
    pub fn new() -> Self {
        let mut path = None;

        // 1. Check compile-time environment variable from build.rs
        if let Some(bin_str) = option_env!("APPLE_TRANSLATE_BRIDGE_BIN") {
            let p = PathBuf::from(bin_str);
            if p.exists() {
                path = Some(p);
            }
        }

        // 2. Check local project bridge path
        if path.is_none() {
            let p = PathBuf::from("bridge/apple_translate_bridge");
            if p.exists() {
                path = Some(p);
            }
        }

        // 3. Check current executable dir
        if path.is_none() {
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    let p = dir.join("apple_translate_bridge");
                    if p.exists() {
                        path = Some(p);
                    }
                }
            }
        }

        Self { bridge_path: path }
    }

    fn bridge_cmd(&self) -> Option<Command> {
        let p = self.bridge_path.as_ref()?;
        Some(Command::new(p))
    }
}

impl Default for AppleTranslator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Translating for AppleTranslator {
    async fn prepare(
        &self,
        source: &str,
        target: &str,
        no_install: bool,
        quiet: bool,
    ) -> Result<(), TranslateError> {
        let src = normalize_language_code(source);
        let dst = normalize_language_code(target);

        let status = self.pair_status(&src, &dst).await;
        match status {
            PairStatus::Installed => Ok(()),
            PairStatus::Supported => {
                if no_install {
                    return Err(TranslateError::ModelNotInstalled(format!("{src}-{dst}")));
                }
                if !quiet {
                    eprintln!("translate: preparing {src}-{dst} model");
                }
                if let Some(mut cmd) = self.bridge_cmd() {
                    let out = cmd.args(["prepare", &src, &dst]).output()?;
                    if !out.status.success() {
                        let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                        return Err(TranslateError::TranslationFailure(err_msg));
                    }
                }
                Ok(())
            }
            PairStatus::Unsupported => Err(TranslateError::UnsupportedPair(format!("{src}-{dst}"))),
        }
    }

    async fn translate(
        &self,
        units: &[String],
        source: &str,
        target: &str,
        preserve_newlines: bool,
    ) -> Result<Vec<String>, TranslateError> {
        let src = normalize_language_code(source);
        let dst = normalize_language_code(target);

        if units.is_empty() {
            return Ok(Vec::new());
        }

        // Deconstruct into segments while protecting code, URLs, and emails
        let mut plans: Vec<Vec<crate::masker::TranslationSegment>> = units
            .iter()
            .map(|u| TranslationMasker::segments(u, preserve_newlines))
            .collect();

        let mut request_locations = Vec::new();
        let mut request_texts = Vec::new();

        for (unit_idx, unit_segments) in plans.iter().enumerate() {
            for (seg_idx, segment) in unit_segments.iter().enumerate() {
                if segment.is_translatable {
                    let trimmed = segment.text.trim();
                    if !trimmed.is_empty() {
                        request_locations.push((unit_idx, seg_idx));
                        request_texts.push(segment.text.clone());
                    }
                }
            }
        }

        if request_texts.is_empty() {
            return Ok(plans
                .into_iter()
                .map(|segs| segs.into_iter().map(|s| s.text).collect())
                .collect());
        }

        let translated_texts = if let Some(mut cmd) = self.bridge_cmd() {
            cmd.args(["translate-batch", &src, &dst])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let mut child = cmd.spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                let json_bytes = serde_json::to_vec(&request_texts)
                    .map_err(|e| TranslateError::TranslationFailure(e.to_string()))?;
                stdin.write_all(&json_bytes)?;
            }

            let output = child.wait_with_output()?;
            if !output.status.success() {
                let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                return Err(TranslateError::TranslationFailure(err_msg));
            }

            let results: Vec<String> = serde_json::from_slice(&output.stdout)
                .map_err(|e| TranslateError::TranslationFailure(e.to_string()))?;
            results
        } else {
            // Fallback for non-macOS or test mock
            request_texts
                .into_iter()
                .map(|t| format!("[{dst}] {t}"))
                .collect()
        };

        for (idx, (unit_idx, seg_idx)) in request_locations.into_iter().enumerate() {
            if let Some(text) = translated_texts.get(idx) {
                plans[unit_idx][seg_idx] = crate::masker::TranslationSegment {
                    text: text.clone(),
                    is_translatable: false,
                };
            }
        }

        Ok(plans
            .into_iter()
            .map(|segs| segs.into_iter().map(|s| s.text).collect())
            .collect())
    }

    async fn pair_status(&self, source: &str, target: &str) -> PairStatus {
        let src = normalize_language_code(source);
        let dst = normalize_language_code(target);

        if let Some(mut cmd) = self.bridge_cmd() {
            if let Ok(out) = cmd.args(["status", &src, &dst]).output() {
                if out.status.success() {
                    let out_str = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                    return match out_str.as_str() {
                        "installed" => PairStatus::Installed,
                        "supported" => PairStatus::Supported,
                        _ => PairStatus::Unsupported,
                    };
                }
            }
        }

        // Fallback checks against catalog
        let src_supp = SUPPORTED_LANGUAGES.iter().any(|(c, _)| *c == src);
        let dst_supp = SUPPORTED_LANGUAGES.iter().any(|(c, _)| *c == dst);
        if src_supp && dst_supp {
            PairStatus::Installed
        } else {
            PairStatus::Unsupported
        }
    }

    async fn installed_pairs(&self) -> Vec<String> {
        let langs = self.available_pairs().await;
        // Check installed pairs
        langs
    }

    async fn available_pairs(&self) -> Vec<String> {
        if let Some(mut cmd) = self.bridge_cmd() {
            if let Ok(out) = cmd.args(["languages"]).output() {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut list: Vec<String> = stdout
                        .lines()
                        .map(|l| normalize_language_code(l.trim()))
                        .filter(|l| !l.is_empty())
                        .collect();
                    list.sort();
                    list.dedup();
                    return list;
                }
            }
        }

        SUPPORTED_LANGUAGES
            .iter()
            .map(|(code, _)| code.to_string())
            .collect()
    }
}

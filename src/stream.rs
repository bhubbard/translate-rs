use crate::detect::LanguageDetector;
use crate::engine::Translating;
use crate::error::TranslateError;
use crate::output::OutputWriter;
use crate::types::{normalize_language_code, DetectionResult, TranslationRecord};
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamToken {
    Unit(String),
    Literal(String),
}

#[derive(Debug, Default)]
pub struct UTF8StreamDecoder {
    pending: Vec<u8>,
}

impl UTF8StreamDecoder {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<String, TranslateError> {
        self.pending.extend_from_slice(data);
        if self.pending.is_empty() {
            return Ok(String::new());
        }

        let max_carry = 3.min(self.pending.len());
        for carry in 0..=max_carry {
            let prefix_count = self.pending.len() - carry;
            if prefix_count == 0 {
                continue;
            }
            if let Ok(s) = std::str::from_utf8(&self.pending[..prefix_count]) {
                let decoded = s.to_string();
                self.pending.drain(..prefix_count);
                return Ok(decoded);
            }
        }

        if self.pending.len() <= 3 {
            return Ok(String::new());
        }

        Err(TranslateError::Input("input is not valid UTF-8".into()))
    }

    pub fn finish(&mut self) -> Result<String, TranslateError> {
        if self.pending.is_empty() {
            return Ok(String::new());
        }
        let s = std::str::from_utf8(&self.pending)
            .map_err(|_| TranslateError::Input("input ended with incomplete or invalid UTF-8".into()))?;
        let res = s.to_string();
        self.pending.clear();
        Ok(res)
    }
}

#[derive(Debug, Clone, Default)]
pub struct LineSplitter {
    buffer: String,
}

impl LineSplitter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn feed(&mut self, chunk: &str) -> Vec<StreamToken> {
        self.buffer.push_str(chunk);
        let mut tokens = Vec::new();

        while let Some(pos) = self.buffer.find('\n') {
            let line = self.buffer[..pos].to_string();
            tokens.push(StreamToken::Unit(line));
            tokens.push(StreamToken::Literal("\n".to_string()));
            self.buffer.drain(..=pos);
        }

        tokens
    }

    pub fn finish(&mut self) -> Vec<StreamToken> {
        if self.buffer.is_empty() {
            Vec::new()
        } else {
            let line = std::mem::take(&mut self.buffer);
            vec![StreamToken::Unit(line)]
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParagraphSplitter {
    buffer: String,
}

impl ParagraphSplitter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn feed(&mut self, chunk: &str) -> Vec<StreamToken> {
        self.buffer.push_str(chunk);
        let mut tokens = Vec::new();

        while let Some(range) = self.first_blank_line_range() {
            let paragraph = self.buffer[..range.start].to_string();
            let sep = self.buffer[range.clone()].to_string();
            if !paragraph.is_empty() {
                tokens.push(StreamToken::Unit(paragraph));
            }
            tokens.push(StreamToken::Literal(sep));
            self.buffer.drain(..range.end);
        }

        tokens
    }

    pub fn finish(&mut self) -> Vec<StreamToken> {
        if self.buffer.is_empty() {
            Vec::new()
        } else {
            let paragraph = std::mem::take(&mut self.buffer);
            vec![StreamToken::Unit(paragraph)]
        }
    }

    fn first_blank_line_range(&self) -> Option<std::ops::Range<usize>> {
        let bytes = self.buffer.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if bytes[i] == b'\n' {
                let mut probe = i + 1;
                while probe < len && (bytes[probe] == b' ' || bytes[probe] == b'\t' || bytes[probe] == b'\r') {
                    probe += 1;
                }
                if probe < len && bytes[probe] == b'\n' {
                    return Some(i..probe + 1);
                }
            }
            i += 1;
        }
        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamProcessor {
    chunk_size: usize,
}

impl StreamProcessor {
    pub fn new() -> Self {
        Self {
            chunk_size: 64 * 1024,
        }
    }

    pub async fn process<R: Read>(
        &self,
        mut reader: R,
        source_override: Option<&str>,
        target_code: &str,
        hints: &[String],
        translator: &(dyn Translating + Send + Sync),
        writer: &mut OutputWriter,
        no_install: bool,
        quiet: bool,
        preserve_newlines: bool,
        batch: bool,
    ) -> Result<(), TranslateError> {
        let mut decoder = UTF8StreamDecoder::new();
        let mut first_buf = vec![0u8; self.chunk_size];
        let bytes_read = reader.read(&mut first_buf)?;
        if bytes_read == 0 {
            return Ok(());
        }

        let first_chunk = decoder.decode(&first_buf[..bytes_read])?;

        let detection = if let Some(src) = source_override {
            DetectionResult {
                language_code: normalize_language_code(src),
                confidence: 1.0,
            }
        } else {
            let detector = LanguageDetector::new(hints);
            detector.detect(&first_chunk)?
        };

        let src_code = normalize_language_code(&detection.language_code);
        let dst_code = normalize_language_code(target_code);

        translator
            .prepare(&src_code, &dst_code, no_install, quiet)
            .await?;

        if batch {
            let mut splitter = LineSplitter::new();
            self.emit(
                splitter.feed(&first_chunk),
                &src_code,
                &dst_code,
                &detection,
                translator,
                writer,
                preserve_newlines,
            )
            .await?;

            let mut buf = vec![0u8; self.chunk_size];
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                let chunk = decoder.decode(&buf[..n])?;
                self.emit(
                    splitter.feed(&chunk),
                    &src_code,
                    &dst_code,
                    &detection,
                    translator,
                    writer,
                    preserve_newlines,
                )
                .await?;
            }

            let tail = decoder.finish()?;
            if !tail.is_empty() {
                self.emit(
                    splitter.feed(&tail),
                    &src_code,
                    &dst_code,
                    &detection,
                    translator,
                    writer,
                    preserve_newlines,
                )
                .await?;
            }

            self.emit(
                splitter.finish(),
                &src_code,
                &dst_code,
                &detection,
                translator,
                writer,
                preserve_newlines,
            )
            .await?;
        } else {
            let mut splitter = ParagraphSplitter::new();
            self.emit(
                splitter.feed(&first_chunk),
                &src_code,
                &dst_code,
                &detection,
                translator,
                writer,
                preserve_newlines,
            )
            .await?;

            let mut buf = vec![0u8; self.chunk_size];
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                let chunk = decoder.decode(&buf[..n])?;
                self.emit(
                    splitter.feed(&chunk),
                    &src_code,
                    &dst_code,
                    &detection,
                    translator,
                    writer,
                    preserve_newlines,
                )
                .await?;
            }

            let tail = decoder.finish()?;
            if !tail.is_empty() {
                self.emit(
                    splitter.feed(&tail),
                    &src_code,
                    &dst_code,
                    &detection,
                    translator,
                    writer,
                    preserve_newlines,
                )
                .await?;
            }

            self.emit(
                splitter.finish(),
                &src_code,
                &dst_code,
                &detection,
                translator,
                writer,
                preserve_newlines,
            )
            .await?;
        }

        Ok(())
    }

    async fn emit(
        &self,
        tokens: Vec<StreamToken>,
        src_code: &str,
        dst_code: &str,
        detection: &DetectionResult,
        translator: &(dyn Translating + Send + Sync),
        writer: &mut OutputWriter,
        preserve_newlines: bool,
    ) -> Result<(), TranslateError> {
        if tokens.is_empty() {
            return Ok(());
        }

        let unit_texts: Vec<String> = tokens
            .iter()
            .filter_map(|t| match t {
                StreamToken::Unit(text) => Some(text.clone()),
                _ => None,
            })
            .collect();

        let translated = translator
            .translate(&unit_texts, src_code, dst_code, preserve_newlines)
            .await?;

        let mut trans_idx = 0;
        for token in tokens {
            match token {
                StreamToken::Literal(lit) => {
                    writer.write_literal(&lit)?;
                }
                StreamToken::Unit(src) => {
                    let dst = translated
                        .get(trans_idx)
                        .cloned()
                        .unwrap_or_else(|| src.clone());
                    trans_idx += 1;

                    writer.write_record(&TranslationRecord {
                        from: src_code.to_string(),
                        to: dst_code.to_string(),
                        src,
                        dst,
                        conf: detection.confidence,
                    })?;
                }
            }
        }

        Ok(())
    }
}

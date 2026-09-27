use regex::Regex;
use std::ops::Range;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationSegment {
    pub text: String,
    pub is_translatable: bool,
}

static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(https?://[^\s<>"']+|www\.[^\s<>"']+)"#).unwrap()
});

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}"#).unwrap()
});

pub struct TranslationMasker;

impl TranslationMasker {
    pub fn segments(text: &str, preserve_newlines: bool) -> Vec<TranslationSegment> {
        if text.is_empty() {
            return vec![TranslationSegment {
                text: String::new(),
                is_translatable: false,
            }];
        }

        let protected_ranges = Self::merged_protected_ranges(text);
        let mut output = Vec::new();
        let mut cursor = 0;

        for range in protected_ranges {
            if cursor < range.start {
                Self::append_translatable(
                    &text[cursor..range.start],
                    preserve_newlines,
                    &mut output,
                );
            }

            output.push(TranslationSegment {
                text: text[range.clone()].to_string(),
                is_translatable: false,
            });

            cursor = range.end;
        }

        if cursor < text.len() {
            Self::append_translatable(&text[cursor..], preserve_newlines, &mut output);
        }

        output
    }

    fn append_translatable(text: &str, preserve_newlines: bool, output: &mut Vec<TranslationSegment>) {
        if !preserve_newlines {
            output.push(TranslationSegment {
                text: text.to_string(),
                is_translatable: true,
            });
            return;
        }

        let mut buffer = String::new();
        let mut newline_buffer = String::new();

        let flush_buffer = |buf: &mut String, out: &mut Vec<TranslationSegment>| {
            if !buf.is_empty() {
                out.push(TranslationSegment {
                    text: buf.clone(),
                    is_translatable: true,
                });
                buf.clear();
            }
        };

        let flush_newlines = |nl: &mut String, out: &mut Vec<TranslationSegment>| {
            if !nl.is_empty() {
                out.push(TranslationSegment {
                    text: nl.clone(),
                    is_translatable: false,
                });
                nl.clear();
            }
        };

        for ch in text.chars() {
            if ch == '\n' || ch == '\r' {
                flush_buffer(&mut buffer, output);
                newline_buffer.push(ch);
            } else {
                flush_newlines(&mut newline_buffer, output);
                buffer.push(ch);
            }
        }

        flush_buffer(&mut buffer, output);
        flush_newlines(&mut newline_buffer, output);
    }

    fn merged_protected_ranges(text: &str) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        ranges.extend(Self::backtick_ranges(text));

        for mat in URL_REGEX.find_iter(text) {
            ranges.push(mat.range());
        }

        for mat in EMAIL_REGEX.find_iter(text) {
            ranges.push(mat.range());
        }

        ranges.sort_by(|a, b| {
            if a.start == b.start {
                a.end.cmp(&b.end)
            } else {
                a.start.cmp(&b.start)
            }
        });

        let mut merged: Vec<Range<usize>> = Vec::new();

        for range in ranges {
            if let Some(last) = merged.last_mut() {
                if range.start <= last.end {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }

        merged
    }

    fn backtick_ranges(text: &str) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let bytes = text.as_bytes();
        let mut index = 0;
        let len = bytes.len();

        while index < len {
            if bytes[index] != b'`' {
                index += 1;
                continue;
            }

            let start = index;

            // Triple backtick fenced code block
            if index + 2 < len && bytes[index + 1] == b'`' && bytes[index + 2] == b'`' {
                let body_start = index + 3;
                if let Some(close_offset) = text[body_start..].find("```") {
                    let end = body_start + close_offset + 3;
                    ranges.push(start..end);
                    index = end;
                } else {
                    ranges.push(start..len);
                    break;
                }
            } else {
                // Inline backtick
                let body_start = index + 1;
                if let Some(close_offset) = text[body_start..].find('`') {
                    let end = body_start + close_offset + 1;
                    ranges.push(start..end);
                    index = end;
                } else {
                    ranges.push(start..len);
                    break;
                }
            }
        }

        ranges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtick_masking() {
        let text = "Translate this: `const x = 10;` but keep code intact.";
        let segments = TranslationMasker::segments(text, false);
        assert_eq!(segments.len(), 3);
        assert!(segments[0].is_translatable);
        assert!(!segments[1].is_translatable);
        assert_eq!(segments[1].text, "`const x = 10;`");
        assert!(segments[2].is_translatable);
    }

    #[test]
    fn test_url_and_email_masking() {
        let text = "Contact support@example.com or visit https://apple.com for help.";
        let segments = TranslationMasker::segments(text, false);
        let non_translatable: Vec<&str> = segments
            .iter()
            .filter(|s| !s.is_translatable)
            .map(|s| s.text.as_str())
            .collect();
        assert!(non_translatable.contains(&"support@example.com"));
        assert!(non_translatable.contains(&"https://apple.com"));
    }

    #[test]
    fn test_preserve_newlines() {
        let text = "Line 1\nLine 2\n\nLine 3";
        let segments = TranslationMasker::segments(text, true);
        let newlines: Vec<&str> = segments
            .iter()
            .filter(|s| !s.is_translatable)
            .map(|s| s.text.as_str())
            .collect();
        assert!(newlines.contains(&"\n"));
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    fn test_protects_urls_emails_and_backticks() {
        let segments = TranslationMasker::segments(
            "Hallo `code` https://example.com a@b.com Welt",
            true,
        );
        assert!(segments.contains(&TranslationSegment {
            text: "`code`".into(),
            is_translatable: false,
        }));
        assert!(segments.contains(&TranslationSegment {
            text: "https://example.com".into(),
            is_translatable: false,
        }));
        assert!(segments.contains(&TranslationSegment {
            text: "a@b.com".into(),
            is_translatable: false,
        }));
    }

    #[test]
    fn test_protects_fenced_code_blocks() {
        let input = "Hallo\n```swift\nlet x = 1\n```\nWelt";
        let segments = TranslationMasker::segments(input, true);
        let fences: Vec<&TranslationSegment> = segments
            .iter()
            .filter(|s| s.text.contains("```") && !s.is_translatable)
            .collect();
        assert!(!fences.is_empty(), "fenced code block must be protected");
    }

    #[test]
    fn test_protects_unterminated_backtick_run_to_end_of_input() {
        let segments = TranslationMasker::segments("Hallo `unfinished", true);
        let last = segments.last().unwrap();
        assert!(!last.is_translatable);
        assert!(last.text.starts_with('`'));
    }

    #[test]
    fn test_preserve_newlines_splits_translatable_runs() {
        let segments = TranslationMasker::segments("a\n\nb", true);
        let translatables: Vec<&str> = segments
            .iter()
            .filter(|s| s.is_translatable)
            .map(|s| s.text.as_str())
            .collect();
        let literals: String = segments
            .iter()
            .filter(|s| !s.is_translatable)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(translatables, vec!["a", "b"]);
        assert_eq!(literals, "\n\n");
    }

    #[test]
    fn test_no_preserve_newlines_keeps_contiguous() {
        let segments = TranslationMasker::segments("a\n\nb", false);
        assert_eq!(segments.len(), 1);
        assert!(segments[0].is_translatable);
        assert_eq!(segments[0].text, "a\n\nb");
    }

    #[test]
    fn test_round_trip_reassembly_is_lossless() {
        let originals = [
            "Hallo Welt",
            "click `here` to https://example.com see a@b.com",
            "line one\nline two\n\nparagraph two",
            "no special tokens here at all",
        ];
        for original in originals {
            let pieces: String = TranslationMasker::segments(original, true)
                .into_iter()
                .map(|s| s.text)
                .collect();
            assert_eq!(pieces, original, "masker must preserve every byte");
        }
    }

    #[test]
    fn test_empty_input_returns_empty_segment() {
        let segments = TranslationMasker::segments("", true);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "");
        assert!(!segments[0].is_translatable);
    }
}

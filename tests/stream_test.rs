use translate_rs::stream::{LineSplitter, ParagraphSplitter, StreamToken, UTF8StreamDecoder};

#[test]
fn test_utf8_stream_decoder_handles_split_multibyte_scalar() {
    let mut decoder = UTF8StreamDecoder::new();
    let euro = "€";
    let bytes = euro.as_bytes();
    assert_eq!(bytes.len(), 3);

    let first = decoder.decode(&bytes[..2]).unwrap();
    assert_eq!(first, "");

    let second = decoder.decode(&bytes[2..]).unwrap();
    assert_eq!(second, euro);

    let tail = decoder.finish().unwrap();
    assert_eq!(tail, "");
}

#[test]
fn test_utf8_stream_decoder_empty_inputs() {
    let mut decoder = UTF8StreamDecoder::new();
    assert_eq!(decoder.decode(&[]).unwrap(), "");
    assert_eq!(decoder.finish().unwrap(), "");
}

#[test]
fn test_utf8_stream_decoder_invalid_mid_stream_throws() {
    let mut decoder = UTF8StreamDecoder::new();
    let res = decoder.decode(&[0xC0, 0xC1, 0xC2, 0xC3, 0xC4]);
    assert!(res.is_err());
}

#[test]
fn test_paragraph_splitter_emits_paragraphs_and_keeps_literal_separator() {
    let mut splitter = ParagraphSplitter::new();
    let tokens = splitter.feed("hallo\n\nwelt");
    let units: Vec<String> = tokens
        .iter()
        .filter_map(|t| match t {
            StreamToken::Unit(u) => Some(u.clone()),
            _ => None,
        })
        .collect();
    let literals: String = tokens
        .iter()
        .filter_map(|t| match t {
            StreamToken::Literal(l) => Some(l.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(units, vec!["hallo"]);
    assert_eq!(literals, "\n\n");

    let tail = splitter.finish();
    assert_eq!(tail.len(), 1);
    match &tail[0] {
        StreamToken::Unit(u) => assert_eq!(u, "welt"),
        _ => panic!("expected unit token in finish()"),
    }
}

#[test]
fn test_paragraph_splitter_handles_multiple_paragraphs() {
    let mut splitter = ParagraphSplitter::new();
    let tokens = splitter.feed("a\n\nb\n\nc\n\n");
    let units: Vec<String> = tokens
        .iter()
        .filter_map(|t| match t {
            StreamToken::Unit(u) => Some(u.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(units, vec!["a", "b", "c"]);
    assert!(splitter.finish().is_empty());
}

#[test]
fn test_line_splitter_emits_one_token_per_newline() {
    let mut splitter = LineSplitter::new();
    let tokens = splitter.feed("alpha\nbeta\ngamma");
    let units: Vec<String> = tokens
        .iter()
        .filter_map(|t| match t {
            StreamToken::Unit(u) => Some(u.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(units, vec!["alpha", "beta"]);

    let tail = splitter.finish();
    assert_eq!(tail.len(), 1);
    match &tail[0] {
        StreamToken::Unit(u) => assert_eq!(u, "gamma"),
        _ => panic!("expected trailing unit"),
    }
}

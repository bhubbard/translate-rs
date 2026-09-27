use translate_rs::output::{format_number, stable_json_record};
use translate_rs::types::{parse_language_hints, TranslationRecord};

#[test]
fn test_record_shape() {
    let record = TranslationRecord {
        from: "de".into(),
        to: "en".into(),
        src: "Hallo".into(),
        dst: "Hello".into(),
        conf: 0.97,
    };

    assert_eq!(
        stable_json_record(&record),
        r#"{"from":"de","to":"en","src":"Hallo","dst":"Hello","conf":0.97}"#
    );
}

#[test]
fn test_string_escapes_quotes_backslashes_and_control() {
    let record = TranslationRecord {
        from: "en".into(),
        to: "de".into(),
        src: "say \"hi\"\n".into(),
        dst: "sag \"hallo\"\n".into(),
        conf: 0.5,
    };
    let json = stable_json_record(&record);
    assert!(json.contains(r#""src":"say \"hi\"\n""#));
    assert!(json.contains(r#""dst":"sag \"hallo\"\n""#));
}

#[test]
fn test_number_format_trims_trailing_zeros() {
    assert_eq!(format_number(0.97), "0.97");
    assert_eq!(format_number(0.5), "0.5");
    assert_eq!(format_number(1.0), "1");
    assert_eq!(format_number(0.123456), "0.123456");
}

#[test]
fn test_number_format_handles_non_finite_as_null() {
    assert_eq!(format_number(f64::NAN), "null");
    assert_eq!(format_number(f64::INFINITY), "null");
}

#[test]
fn test_language_list_parse() {
    assert_eq!(parse_language_hints(None), Vec::<String>::new());
    assert_eq!(parse_language_hints(Some("")), Vec::<String>::new());
    assert_eq!(parse_language_hints(Some("de")), vec!["de"]);
    assert_eq!(parse_language_hints(Some("de, fr ,en")), vec!["de", "fr", "en"]);
    assert_eq!(parse_language_hints(Some("de,,en")), vec!["de", "en"]);
}

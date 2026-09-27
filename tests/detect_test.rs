use translate_rs::detect::LanguageDetector;

#[test]
fn test_detection_known_languages() {
    let detector = LanguageDetector::new(&[]);

    let german = detector.detect("Das ist ein kurzer deutscher Satz über die Welt.").unwrap();
    assert_eq!(&german.language_code[..2], "de");
    assert!(german.confidence > 0.0);

    let french = detector.detect("Ceci est une phrase française avec plusieurs mots.").unwrap();
    assert_eq!(&french.language_code[..2], "fr");

    let english = detector.detect("This is a short English sentence about the world.").unwrap();
    assert_eq!(&english.language_code[..2], "en");
}

#[test]
fn test_detection_empty_input_throws() {
    let detector = LanguageDetector::new(&[]);
    assert!(detector.detect("").is_err());
    assert!(detector.detect("   \n  \t  ").is_err());
}

#[test]
fn test_detection_with_hints_constrains_result() {
    let hints = vec!["de".to_string(), "fr".to_string()];
    let detector = LanguageDetector::new(&hints);
    let result = detector.detect("Bonjour le monde, comment ça va aujourd'hui ?").unwrap();
    assert!(vec!["de", "fr"].contains(&&result.language_code[..2]));
}

use translate_rs::error::TranslateError;

#[test]
fn test_exit_codes() {
    assert_eq!(TranslateError::Usage("x".into()).exit_code(), 1);
    assert_eq!(TranslateError::Input("x".into()).exit_code(), 1);
    assert_eq!(TranslateError::TranslationFailure("x".into()).exit_code(), 2);
    assert_eq!(TranslateError::Io("x".into()).exit_code(), 2);
    assert_eq!(TranslateError::UnsupportedOS.exit_code(), 3);
    assert_eq!(TranslateError::ModelNotInstalled("de-en".into()).exit_code(), 4);
    assert_eq!(TranslateError::UnsupportedPair("de-zz".into()).exit_code(), 5);
}

#[test]
fn test_descriptions_are_prefixed_with_tool_name() {
    assert!(TranslateError::Usage("foo".into()).to_string().starts_with("translate: "));
    assert!(TranslateError::Input("foo".into()).to_string().starts_with("translate: "));
    assert!(TranslateError::TranslationFailure("oops".into()).to_string().contains("translation failed"));
    assert!(TranslateError::UnsupportedOS.to_string().contains("macOS 26"));
    assert!(TranslateError::ModelNotInstalled("de-en".into()).to_string().contains("--install"));
    assert!(TranslateError::UnsupportedPair("de-zz".into()).to_string().contains("--available"));
}

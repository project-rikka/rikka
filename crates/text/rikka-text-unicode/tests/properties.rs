use rikka_text_unicode::{
    BidiClass, GeneralCategory, JoiningType, LineBreakClass, Script, properties,
};

#[test]
fn properties_resolve_in_one_lookup() {
    let word = properties('\u{0644}');
    assert_eq!(word.general_category(), GeneralCategory::OtherLetter);
    assert_eq!(word.script(), Script::Arabic);
    assert_eq!(word.bidi_class(), BidiClass::ArabicLetter);
    assert_eq!(word.joining_type(), JoiningType::DualJoining);
    assert_eq!(word.line_break_class(), LineBreakClass::Alphabetic);
    assert_eq!(word.combining_class(), 0);
    assert!(!word.is_emoji());
}

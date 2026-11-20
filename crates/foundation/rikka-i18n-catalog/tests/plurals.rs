use rikka_i18n_catalog::PluralCategory::{self, Few, Many, One, Other, Two, Zero};
use rikka_i18n_catalog::{PluralKind, PluralOperands};

#[test]
fn cardinal_categories_follow_cldr() {
    let cases: &[(&str, &str, PluralCategory)] = &[
        ("en", "0", Other),
        ("en", "1", One),
        ("en", "2", Other),
        ("en", "1.0", Other),
        ("ja", "1", Other),
        ("ja", "11", Other),
        ("ru", "1", One),
        ("ru", "21", One),
        ("ru", "11", Many),
        ("ru", "2", Few),
        ("ru", "24", Few),
        ("ru", "5", Many),
        ("ru", "1.5", Other),
        ("pl", "3", Few),
        ("pl", "5", Many),
        ("cs", "3", Few),
        ("cs", "1.5", Many),
        ("ar", "0", Zero),
        ("ar", "1", One),
        ("ar", "2", Two),
        ("ar", "5", Few),
        ("ar", "15", Many),
        ("ar", "100", Other),
        ("cy", "6", Many),
        ("fr", "0", One),
        ("fr", "2", Other),
        ("qqq", "1", One),
        ("qqq", "7", Other),
    ];
    for &(language, number, expected) in cases {
        let operands = PluralOperands::decimal(number).expect("a decimal");
        assert_eq!(
            PluralCategory::select(language, PluralKind::Cardinal, operands),
            expected,
            "{number} in {language}"
        );
    }
    assert!(PluralOperands::decimal("1e5").is_none());
}

#[test]
fn english_ordinals_rank_correctly() {
    for (value, expected) in [
        (1, One),
        (2, Two),
        (3, Few),
        (4, Other),
        (11, Other),
        (21, One),
    ] {
        assert_eq!(
            PluralCategory::select("en", PluralKind::Ordinal, PluralOperands::integer(value)),
            expected,
            "{value}"
        );
    }
}

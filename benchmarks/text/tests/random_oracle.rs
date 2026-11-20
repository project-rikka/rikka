use std::thread;

use text_shaping::{check_random, profile};

const TEXTS_PER_FONT: usize = 1000;

const PROFILES: &[&str] = &[
    "latin",
    "greek-cyrillic",
    "arabic",
    "hebrew",
    "thai-lao",
    "hangul",
    "syriac",
    "nko",
    "mongolian",
    "cjk",
    "emoji",
    "vietnamese",
    "devanagari",
    "bengali",
    "gurmukhi",
    "gujarati",
    "oriya",
    "tamil",
    "telugu",
    "kannada",
    "malayalam",
    "sinhala",
    "khmer",
    "myanmar",
    "tibetan",
    "javanese",
    "balinese",
    "sundanese",
    "tai-tham",
    "chakma",
    "newa",
    "sharada-siddham",
    "north-brahmic",
    "brahmi-grantha",
    "southeast-asian",
    "himalayan",
    "gondi-and-squares",
    "philippine",
    "adlam",
    "hanifi-rohingya",
    "sogdian-family",
    "modern-use",
    "duployan-hieroglyphs",
    "tifinagh-and-kharoshthi",
    "phantom-advances",
];

fn differences(profile_name: &str) -> Option<String> {
    let profile = profile(profile_name).expect("profile should exist");
    let report = check_random(profile, TEXTS_PER_FONT, 0x5EED ^ profile_name.len() as u64);
    println!(
        "{profile_name}: {} of {} random comparisons identical, {} skipped, {} hit proven harfrust bugs",
        report.passed,
        report.total(),
        report.skipped,
        report.harfrust_bugs
    );
    if report.passed == 0 {
        return Some(format!("no {profile_name} font is installed"));
    }
    (!report.failures.is_empty()).then(|| {
        let shown: Vec<&str> = report
            .failures
            .iter()
            .take(12)
            .map(String::as_str)
            .collect();
        format!(
            "{} of {} random {profile_name} texts differ from harfrust:\n{}",
            report.failures.len(),
            report.total(),
            shown.join("\n")
        )
    })
}

#[test]
fn random_texts_match_harfrust() {
    let failures: Vec<String> = thread::scope(|scope| {
        let checks: Vec<_> = PROFILES
            .iter()
            .map(|name| scope.spawn(move || differences(name)))
            .collect();
        checks
            .into_iter()
            .filter_map(|check| check.join().expect("comparison thread should finish"))
            .collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

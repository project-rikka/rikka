use std::thread;

use text_shaping::check_group;

const GROUPS: &[&str] = &[
    "latin",
    "vietnamese",
    "greek",
    "cyrillic",
    "variable",
    "arabic",
    "urdu",
    "persian",
    "syriac",
    "nko",
    "mongolian",
    "joining",
    "hebrew",
    "thai",
    "lao",
    "hangul",
    "cjk",
    "emoji",
    "mixed",
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
    "hieroglyphs",
    "use",
    "phantom-advances",
];

fn differences(group: &str) -> Option<String> {
    let report = check_group(group);
    println!(
        "{group}: {} of {} comparisons identical, {} skipped, {} hit proven harfrust bugs",
        report.passed,
        report.total(),
        report.skipped,
        report.harfrust_bugs
    );
    if report.passed == 0 {
        return Some(format!("no {group} font is installed"));
    }
    (!report.failures.is_empty()).then(|| {
        format!(
            "{} of {} {group} comparisons differ from harfrust:\n{}",
            report.failures.len(),
            report.total(),
            report.failures.join("\n")
        )
    })
}

#[test]
fn script_groups_match_harfrust() {
    let failures: Vec<String> = thread::scope(|scope| {
        let checks: Vec<_> = GROUPS
            .iter()
            .map(|group| scope.spawn(move || differences(group)))
            .collect();
        checks
            .into_iter()
            .filter_map(|check| check.join().expect("comparison thread should finish"))
            .collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

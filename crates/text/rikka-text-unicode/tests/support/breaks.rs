use super::codepoints::parse_codepoints;

pub(crate) struct BreakCase {
    pub(crate) line: usize,
    pub(crate) text: String,
    pub(crate) breaks: Vec<usize>,
}

pub(crate) fn break_cases(data: &str) -> Vec<BreakCase> {
    let mut cases = Vec::new();
    for (index, line) in data.lines().enumerate() {
        let content = line.split('#').next().unwrap_or("").trim();
        if content.is_empty() {
            continue;
        }
        let mut text = String::new();
        let mut breaks = Vec::new();
        for token in content.split_whitespace() {
            match token {
                "÷" if !text.is_empty() => breaks.push(text.len()),
                "÷" | "×" => {}
                hex => text.push_str(&parse_codepoints(hex)),
            }
        }
        cases.push(BreakCase {
            line: index + 1,
            text,
            breaks,
        });
    }
    cases
}

pub(crate) fn assert_breaks(name: &str, data: &str, segment: impl Fn(&str) -> Vec<usize>) {
    let cases = break_cases(data);
    assert!(!cases.is_empty());
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let actual = segment(&case.text);
            (actual != case.breaks).then(|| {
                format!(
                    "line {}: {:?} expected {:?}, got {:?}",
                    case.line, case.text, case.breaks, actual
                )
            })
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{name}: {} of {} cases fail:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

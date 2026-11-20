use crate::support::bidi::{parse_levels, parse_order, resolve};
use crate::support::codepoints::parse_codepoints;
use rikka_text_unicode::{
    BaseDirection, Bidi, Level, VisualLine, logical_to_visual, visual_to_logical,
};

fn representative(class: &str) -> char {
    match class {
        "L" => 'a',
        "R" => '\u{05D0}',
        "AL" => '\u{0627}',
        "EN" => '1',
        "ES" => '+',
        "ET" => '$',
        "AN" => '\u{0660}',
        "CS" => ',',
        "NSM" => '\u{0300}',
        "BN" => '\u{00AD}',
        "B" => '\u{2029}',
        "S" => '\t',
        "WS" => ' ',
        "ON" => '!',
        "LRE" => '\u{202A}',
        "RLE" => '\u{202B}',
        "PDF" => '\u{202C}',
        "LRO" => '\u{202D}',
        "RLO" => '\u{202E}',
        "LRI" => '\u{2066}',
        "RLI" => '\u{2067}',
        "FSI" => '\u{2068}',
        "PDI" => '\u{2069}',
        other => panic!("unknown bidi class `{other}`"),
    }
}

#[test]
fn bidi_class_conformance() {
    let mut bidi = Bidi::new();
    let mut line = VisualLine::new();
    let mut levels = Vec::new();
    let mut order = Vec::new();
    let mut cases = 0;
    let mut failures = Vec::new();
    for (index, row) in include_str!("../ucd/BidiTest.txt").lines().enumerate() {
        let row = row.split('#').next().unwrap_or("").trim();
        if let Some(field) = row.strip_prefix("@Levels:") {
            levels = parse_levels(field);
            continue;
        }
        if let Some(field) = row.strip_prefix("@Reorder:") {
            order = parse_order(field);
            continue;
        }
        let Some((classes, directions)) = row.split_once(';') else {
            continue;
        };
        let text: String = classes.split_whitespace().map(representative).collect();
        let directions = u8::from_str_radix(directions.trim(), 16).unwrap_or(0);
        for (bit, direction) in [
            (1, BaseDirection::Auto),
            (2, BaseDirection::LeftToRight),
            (4, BaseDirection::RightToLeft),
        ] {
            if directions & bit == 0 {
                continue;
            }
            cases += 1;
            let resolution = resolve(&mut bidi, &mut line, &text, direction);
            if resolution.levels != levels || resolution.order != order {
                failures.push(format!(
                    "line {}: {classes} ({direction:?}) expected {levels:?} {order:?}, got {:?} {:?}",
                    index + 1,
                    resolution.levels,
                    resolution.order
                ));
            }
        }
    }
    assert!(cases > 0);
    assert!(
        failures.is_empty(),
        "{} of {cases} cases fail:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

#[test]
fn bidi_character_conformance() {
    let mut bidi = Bidi::new();
    let mut line = VisualLine::new();
    let mut cases = 0;
    let mut failures = Vec::new();
    for (index, row) in include_str!("../ucd/BidiCharacterTest.txt")
        .lines()
        .enumerate()
    {
        let row = row.split('#').next().unwrap_or("").trim();
        let fields: Vec<&str> = row.split(';').collect();
        let [codepoints, direction, paragraph_level, levels, order] = fields[..] else {
            continue;
        };
        let text = parse_codepoints(codepoints);
        let direction = match direction.trim() {
            "0" => BaseDirection::LeftToRight,
            "1" => BaseDirection::RightToLeft,
            _ => BaseDirection::Auto,
        };
        cases += 1;
        let resolution = resolve(&mut bidi, &mut line, &text, direction);
        let expected_level: u8 = paragraph_level.trim().parse().unwrap_or(u8::MAX);
        let (levels, order) = (parse_levels(levels), parse_order(order));
        if resolution.paragraph_level != expected_level
            || resolution.levels != levels
            || resolution.order != order
        {
            failures.push(format!(
                "line {}: {codepoints} expected {expected_level} {levels:?} {order:?}, got {} {:?} {:?}",
                index + 1,
                resolution.paragraph_level,
                resolution.levels,
                resolution.order
            ));
        }
    }
    assert!(cases > 0);
    assert!(
        failures.is_empty(),
        "{} of {cases} cases fail:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

#[test]
fn visual_line_reverses_rtl_runs() {
    let mut bidi = Bidi::new();
    let mut line = VisualLine::new();
    let text = "abc \u{05D0}\u{05D1} 12 \u{05D2} def";
    bidi.analyze(text, BaseDirection::LeftToRight);
    bidi.reorder_line(0..text.len(), &mut line);
    let runs: Vec<(&str, u8)> = line
        .runs()
        .iter()
        .map(|run| (&text[run.range.clone()], run.level.number()))
        .collect();
    assert_eq!(
        runs,
        [
            ("abc ", 0),
            (" \u{05D2}", 1),
            ("12", 2),
            ("\u{05D0}\u{05D1} ", 1),
            (" def", 0),
        ]
    );
}

#[test]
fn index_maps_are_inverse_permutations() {
    let levels: Vec<Level> = [0, 0, 1, 1, 2, 2, 1, 0]
        .into_iter()
        .filter_map(Level::new)
        .collect();
    let mut visual = Vec::new();
    let mut logical = Vec::new();
    visual_to_logical(&levels, &mut visual);
    logical_to_visual(&levels, &mut logical);
    assert_eq!(visual, [0, 1, 6, 4, 5, 3, 2, 7]);
    for (position, &index) in visual.iter().enumerate() {
        assert_eq!(logical[index], position);
    }
}

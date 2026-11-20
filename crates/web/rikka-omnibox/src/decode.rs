const UNSAFE_RANGES: &[(char, char)] = &[
    ('\u{00A0}', '\u{00A0}'),
    ('\u{00AD}', '\u{00AD}'),
    ('\u{034F}', '\u{034F}'),
    ('\u{0337}', '\u{0338}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{17B4}', '\u{17B5}'),
    ('\u{180B}', '\u{180F}'),
    ('\u{2000}', '\u{200F}'),
    ('\u{2028}', '\u{202F}'),
    ('\u{2044}', '\u{2044}'),
    ('\u{205F}', '\u{206F}'),
    ('\u{2215}', '\u{2215}'),
    ('\u{2571}', '\u{2571}'),
    ('\u{2800}', '\u{2800}'),
    ('\u{29F8}', '\u{29F8}'),
    ('\u{3000}', '\u{3000}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FF03}', '\u{FF03}'),
    ('\u{FF0F}', '\u{FF0F}'),
    ('\u{FF1F}', '\u{FF1F}'),
    ('\u{FFA0}', '\u{FFA0}'),
    ('\u{FFF0}', '\u{FFFF}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{1F50F}', '\u{1F513}'),
    ('\u{E0000}', '\u{E0FFF}'),
];

fn is_safe(character: char) -> bool {
    if character.is_ascii() {
        return character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '~');
    }
    !character.is_control()
        && !character.is_whitespace()
        && !UNSAFE_RANGES
            .iter()
            .any(|&(start, end)| (start..=end).contains(&character))
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn escaped_at(bytes: &[u8], index: usize) -> Option<u8> {
    if bytes.get(index) != Some(&b'%') {
        return None;
    }
    Some((hex(*bytes.get(index + 1)?)? << 4) | hex(*bytes.get(index + 2)?)?)
}

pub(crate) fn decode_into(target: &mut String, component: &str) {
    let bytes = component.as_bytes();
    let mut index = 0;
    let mut run = Vec::new();
    while index < bytes.len() {
        let Some(first) = escaped_at(bytes, index) else {
            let character = component[index..].chars().next().unwrap_or_default();
            target.push(character);
            index += character.len_utf8();
            continue;
        };
        let start = index;
        run.clear();
        run.push(first);
        index += 3;
        while let Some(byte) = escaped_at(bytes, index) {
            run.push(byte);
            index += 3;
        }
        push_run(target, &run, &component[start..index]);
    }
}

fn push_run(target: &mut String, run: &[u8], original: &str) {
    let mut consumed = 0;
    while consumed < run.len() {
        let width = utf8_width(run[consumed]);
        let decoded = run
            .get(consumed..consumed + width)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|text| text.chars().next())
            .filter(|&character| is_safe(character));
        match decoded {
            Some(character) => {
                target.push(character);
                consumed += width;
            }
            None => {
                target.push_str(&original[consumed * 3..consumed * 3 + 3]);
                consumed += 1;
            }
        }
    }
}

fn utf8_width(byte: u8) -> usize {
    match byte {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

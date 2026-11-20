#[must_use]
pub fn format_count(value: u64) -> String {
    match value {
        0..1_000 => value.to_string(),
        1_000..1_000_000 => format!("{}K", value / 1_000),
        _ => format!("{:.1}M", value as f64 / 1_000_000.0),
    }
}

const GROUP_SEPARATOR: char = ',';

#[must_use]
pub fn group_digits(number: &str) -> String {
    let (sign, unsigned) = match number.strip_prefix('-') {
        Some(unsigned) => ("-", unsigned),
        None => ("", number),
    };
    let (integer, fraction) = match unsigned.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (unsigned, None),
    };
    let mut grouped = String::with_capacity(number.len() + integer.len() / 3);
    grouped.push_str(sign);
    for (index, digit) in integer.char_indices() {
        if index > 0 && (integer.len() - index).is_multiple_of(3) {
            grouped.push(GROUP_SEPARATOR);
        }
        grouped.push(digit);
    }
    if let Some(fraction) = fraction {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
}

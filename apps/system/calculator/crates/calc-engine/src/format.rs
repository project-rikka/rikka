pub(crate) const SIGNIFICANT_DIGITS: usize = 16;

const UPPER: f64 = 1e16;
const LOWER: f64 = 1e-4;

pub const SEPARATOR: char = ',';

pub(crate) fn number(value: f64) -> String {
    if value == 0.0 {
        return String::from("0");
    }

    let magnitude = value.abs();
    if !(LOWER..UPPER).contains(&magnitude) {
        return scientific(value);
    }

    let exponent = magnitude.log10().floor() as i32;
    let decimals = (SIGNIFICANT_DIGITS as i32 - 1 - exponent).clamp(0, 19) as usize;
    group(trim(&format!("{value:.decimals$}")))
}

pub(crate) fn group(text: &str) -> String {
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let (integer, fraction) = match rest.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (rest, None),
    };

    let mut grouped = String::with_capacity(text.len() + integer.len() / 3);
    grouped.push_str(sign);
    for (index, digit) in integer.char_indices() {
        if index > 0 && (integer.len() - index).is_multiple_of(3) {
            grouped.push(SEPARATOR);
        }
        grouped.push(digit);
    }
    if let Some(fraction) = fraction {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
}

fn scientific(value: f64) -> String {
    let precision = SIGNIFICANT_DIGITS - 1;
    let formatted = format!("{value:.precision$e}");
    let (mantissa, exponent) = formatted
        .split_once('e')
        .expect("exponent formatting writes an exponent");
    let sign = if exponent.starts_with('-') { "" } else { "+" };
    format!("{}e{sign}{exponent}", trim(mantissa))
}

fn trim(fixed: &str) -> &str {
    match fixed.contains('.') {
        true => fixed.trim_end_matches('0').trim_end_matches('.'),
        false => fixed,
    }
}

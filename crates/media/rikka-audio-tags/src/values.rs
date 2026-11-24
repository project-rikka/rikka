use crate::tags::{Date, Position};

const R128_STEPS_PER_DB: f32 = 256.0;
const R128_TO_REPLAYGAIN_DB: f32 = 5.0;
const ISRC_LENGTH: usize = 12;

pub(crate) fn position(number: Option<&str>, total: Option<&str>) -> Position {
    let (number, inline_total) = number.map_or((None, None), pair);
    Position {
        number,
        total: total.and_then(leading_number).or(inline_total),
    }
}

fn pair(text: &str) -> (Option<u32>, Option<u32>) {
    match text.split_once('/') {
        Some((number, total)) => (leading_number(number), leading_number(total)),
        None => (leading_number(text), None),
    }
}

fn leading_number(text: &str) -> Option<u32> {
    let text = text.trim();
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    text[..end].parse().ok().filter(|number| *number > 0)
}

pub(crate) fn date(text: &str) -> Option<Date> {
    let text = text.trim();
    let digits = |from: usize, count: usize| -> Option<u32> {
        let part = text.get(from..from + count)?;
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let year = u16::try_from(digits(0, 4)?).ok().filter(|year| *year > 0)?;
    let separated = |at: usize| {
        text.as_bytes()
            .get(at)
            .is_some_and(|byte| matches!(byte, b'-' | b'.' | b'/'))
    };
    let (month_at, day_at) = if separated(4) {
        (5, separated(7).then_some(8))
    } else {
        (4, Some(6))
    };
    let month = digits(month_at, 2)
        .and_then(|month| u8::try_from(month).ok())
        .filter(|month| (1..=12).contains(month));
    let day = month
        .and(day_at)
        .and_then(|at| digits(at, 2))
        .and_then(|day| u8::try_from(day).ok())
        .filter(|day| (1..=31).contains(day));
    Some(Date { year, month, day })
}

pub(crate) fn decibels(text: &str) -> Option<f32> {
    let text = text.trim();
    let number = ["db", "lu", "lufs"]
        .iter()
        .find_map(|unit| {
            let split = text.len().checked_sub(unit.len())?;
            let (number, suffix) = text.split_at_checked(split)?;
            suffix.eq_ignore_ascii_case(unit).then_some(number)
        })
        .unwrap_or(text);
    decimal(number)
}

pub(crate) fn peak(text: &str) -> Option<f32> {
    decimal(text).filter(|peak| *peak >= 0.0)
}

pub(crate) fn r128(text: &str) -> Option<f32> {
    let steps: i32 = text.trim().parse().ok()?;
    Some(steps as f32 / R128_STEPS_PER_DB + R128_TO_REPLAYGAIN_DB)
}

fn decimal(text: &str) -> Option<f32> {
    let text = text.trim().replace(',', ".");
    let text = text.strip_prefix('+').unwrap_or(&text);
    text.parse::<f32>().ok().filter(|value| value.is_finite())
}

pub(crate) fn flag(text: &str) -> bool {
    let text = text.trim();
    ["true", "yes"]
        .iter()
        .any(|word| text.eq_ignore_ascii_case(word))
        || text.parse::<i64>().is_ok_and(|number| number != 0)
}

pub(crate) fn isrc(text: &str) -> Option<String> {
    let code: String = text
        .chars()
        .filter(|character| !matches!(character, '-' | ' '))
        .map(|character| character.to_ascii_uppercase())
        .collect();
    (code.len() == ISRC_LENGTH
        && code
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then_some(code)
}

const MAX_SCALED: f64 = 1e16;

pub(crate) fn rounded(value: f64, digits: i32) -> f64 {
    if value.abs() >= MAX_SCALED {
        return value;
    }
    let power = 10f64.powi(digits);
    (value * power).round_ties_even() / power
}

pub(crate) fn written(value: f64) -> String {
    match value == 0.0 {
        true => "0".to_owned(),
        false => format!("{value}"),
    }
}

pub(crate) fn percent(unit: f64) -> String {
    written(rounded(unit * 100.0, 0))
}

pub(crate) fn decimals(value: f64, digits: i32) -> String {
    written(rounded(value, digits))
}

pub(crate) fn fraction(value: f64, leading_zero: bool) -> String {
    let fixed = format!("{value:.2}");
    let trimmed = match fixed.contains('.') {
        true => fixed.trim_end_matches('0').trim_end_matches('.'),
        false => fixed.as_str(),
    };
    match (leading_zero, trimmed) {
        (false, "0") => String::new(),
        (false, _) => trimmed
            .strip_prefix("0.")
            .map_or_else(|| trimmed.to_owned(), |rest| format!(".{rest}")),
        (true, _) => trimmed.to_owned(),
    }
}

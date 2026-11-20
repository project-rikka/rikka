use std::path::Path;

pub(crate) fn text(path: impl AsRef<Path>) -> Option<String> {
    let mut text = std::fs::read_to_string(path).ok()?;
    text.truncate(text.trim_end().len());
    Some(text)
}

pub(crate) fn number<T: std::str::FromStr>(path: impl AsRef<Path>) -> Option<T> {
    text(path)?.trim().parse().ok()
}

pub(crate) fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim())
    })
}

pub(crate) fn leading_number(value: &str) -> Option<u64> {
    value.split_whitespace().next()?.parse().ok()
}

pub(crate) fn entries(path: impl AsRef<Path>) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    read.flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

pub(crate) fn numbers(line: &str, skip: usize) -> impl Iterator<Item = u64> + '_ {
    line.split_whitespace()
        .skip(skip)
        .map_while(|field| field.parse().ok())
}

pub(crate) fn rate(current: u64, previous: u64, seconds: f32) -> f32 {
    if seconds <= 0.0 {
        return 0.0;
    }
    current.saturating_sub(previous) as f32 / seconds
}

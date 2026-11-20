const GROUP: usize = 5;

pub(crate) fn displayable(digits: &str) -> String {
    digits
        .as_bytes()
        .chunks(GROUP)
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>()
        .join(" ")
}

pub const FALLBACK_RELEASE: &str = "26.05";

#[must_use]
pub fn stable_release(nixos_version: &str) -> Option<String> {
    let version = nixos_version.split_whitespace().next()?;
    let (year, rest) = version.split_once('.')?;
    let month_digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let (month, suffix) = rest.split_at(month_digits);

    let year: u32 = year.parse().ok()?;
    let month: u32 = month.parse().ok()?;
    if month != 5 && month != 11 {
        return None;
    }

    let (year, month) = match (suffix.starts_with("pre"), month) {
        (false, _) => (year, month),
        (true, 11) => (year, 5),
        (true, _) => (year.checked_sub(1)?, 11),
    };
    Some(format!("{year:02}.{month:02}"))
}

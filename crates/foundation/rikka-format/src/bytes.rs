const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];

#[must_use]
pub fn format_bytes(count: u64) -> String {
    if count < 1024 {
        return format!("{count} B");
    }
    let magnitude = (count.ilog2() / 10) as usize;
    let tenths = ((count >> (magnitude * 10 - 10)) * 10 + 512) / 1024;
    match UNITS.get(magnitude + 1) {
        Some(next) if tenths == 1024 * 10 => format!("1.0 {next}"),
        _ => format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[magnitude]),
    }
}

const CONTROL_SHARE: f32 = 0.05;

pub(crate) fn is_probably_text(header: &[u8]) -> bool {
    if header.is_empty() {
        return true;
    }
    if header.contains(&0) {
        return false;
    }
    let control = header
        .iter()
        .filter(|byte| byte.is_ascii_control() && !matches!(byte, b'\t' | b'\n' | b'\r' | 0x0C))
        .count();
    control as f32 <= header.len() as f32 * CONTROL_SHARE
}

use rikka_settings::session::displays::Mode;

pub(crate) fn size(width: u32, height: u32) -> String {
    format!("{width} × {height}")
}

pub(crate) fn refresh(mode: Mode) -> String {
    if mode.refresh.is_multiple_of(1000) {
        format!("{} Hz", mode.refresh / 1000)
    } else {
        format!("{:.2} Hz", f64::from(mode.refresh) / 1000.0)
    }
}

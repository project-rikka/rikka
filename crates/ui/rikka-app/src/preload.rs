use rikka_theme::Theme;
use rikka_ui::TextRun;

const SAMPLE: &str = "0";

pub(crate) fn fonts(theme: &Theme) {
    let run = TextRun::new(SAMPLE, theme.typography().body_size);
    let started = std::thread::Builder::new()
        .name("rikka-font-load".into())
        .spawn(move || {
            rikka_ui::measure_text(&run, None);
        });
    if let Err(error) = started {
        tracing::warn!("failed to start font loading: {error}");
    }
}

use rikka_app::prelude::*;

pub(crate) fn hint(message: impl Into<Label>) -> Container {
    container(
        text(message)
            .caption()
            .align(TextAlign::Center)
            .muted()
            .fill_width(),
    )
    .padding(Inset::xy(12.0, 24.0))
    .fill_width()
}

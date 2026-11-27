use rikka_app::prelude::*;

use crate::shell::Shell;

pub(crate) const BUTTON: &str = "rikka-browser-downloads";
const SIDE: f32 = 18.0;

pub(crate) fn button(shell: &Shell) -> Element {
    let profile = shell.model().profile_here(shell.id());
    let page_services = shell.browser().page_services();
    let downloads = page_services.downloads();
    if downloads.items(profile).is_empty() {
        return Element::empty();
    }
    let activity = downloads.activity(profile);
    let glyph = match (activity.running, activity.fraction) {
        (0, _) => icon(MaterialSymbol::DOWNLOAD)
            .extent(Length::px(SIDE))
            .any(),
        (_, Some(fraction)) => gauge(fraction).extent(Length::px(SIDE)).any(),
        (_, None) => spinner().extent(Length::px(SIDE)).any(),
    };
    rikka_widgets::button(glyph)
        .ghost()
        .icon_only()
        .small()
        .focusless()
        .on_press(|shell: &mut Shell| shell.toggle_downloads())
        .build()
        .keyed(BUTTON)
}

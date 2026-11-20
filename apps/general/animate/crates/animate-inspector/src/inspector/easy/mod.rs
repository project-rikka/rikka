mod automation;
mod key;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::Desk;
use crate::inspector::detail;
use crate::inspector::morphs::morphs;
use crate::inspector::style::style;

pub(super) fn easy(desk: Desk<'_>) -> impl View {
    let keying = match desk.chosen_morph() {
        Some(_) => detail(desk).any(),
        None => key::key(desk).any(),
    };
    column! {
        keying;
        rule();
        if let Some(morphs) = morphs(desk) {
            morphs;
            rule();
        }
        automation::automation(desk);
        rule();
        style(desk);
    }
    .gap(Space::SNUG)
    .fill_width()
}

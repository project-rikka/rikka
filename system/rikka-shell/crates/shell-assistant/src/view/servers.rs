use rikka_chat::servers;
use rikka_ui::{Element, Inset, Styled, View};
use rikka_widgets::{container, scrollable};

use crate::fitting::place_titled;
use crate::panel::Panel;

const EDGE: f32 = 12.0;

pub(super) fn view(panel: &Panel) -> Element {
    scrollable(
        container(servers(panel.servers(), place_titled))
            .padding(Inset::edges(0.0, EDGE, EDGE, EDGE))
            .fill_width(),
    )
    .grow(1.0)
    .fill_width()
    .build()
}

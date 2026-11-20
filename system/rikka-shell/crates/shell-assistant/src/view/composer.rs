use rikka_chat::{Composer, attach, context_fill, outgoing_message};
use rikka_ui::{Align, Element, Space, Styled, View};
use rikka_widgets::{row, spacer};

use crate::chat::ShellChat;
use crate::panel::Panel;

const MODELS: f32 = 160.0;

pub(super) fn view(panel: &Panel) -> impl View {
    let shown = panel.shown();
    Composer::<ShellChat>::new(panel.draft(), controls(panel))
        .thread(shown.thread.id())
        .attached(panel.attached())
        .model(shown.model.as_ref())
        .asking(panel.asking())
        .offering(panel.offering())
        .error(shown.trouble.as_ref())
        .jobs(&shown.jobs, None)
        .panels(panel.models().panels::<ShellChat>(panel.picking()))
        .view()
}

fn controls(panel: &Panel) -> Element {
    let shown = panel.shown();
    row! {
        attach::<ShellChat>(true);
        if !panel.models().is_empty() {
            panel.models().trigger::<ShellChat>(panel.picking(), MODELS);
        }
        spacer().grow(1.0);
        shown
            .model
            .as_ref()
            .and_then(|model| context_fill(shown.thread.context_tokens(), model));
        outgoing_message::<ShellChat>(
            shown.is_answering(),
            panel.draft(),
            !panel.attached().is_empty(),
            panel.is_sending() || !shown.is_ready(),
        )
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .shrink(0.0)
    .fill_width()
    .build()
}

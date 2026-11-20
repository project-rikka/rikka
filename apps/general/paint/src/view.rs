use rikka_app::prelude::*;
use rikka_ui::{Accept, DragAction, DragOffer};
use rikka_widgets::column;

use crate::command::Command;
use crate::paint::Paint;
use crate::{new_image, start, workspace};

const URI_LIST: &str = "text/uri-list";
pub(crate) const STAGE: &str = "paint-stage";

pub(crate) const SIDE_PANEL: f32 = 296.0;

pub(crate) fn view(paint: &Paint) -> impl View {
    let body = match paint.active_tab() {
        Some(tab) => workspace::workspace(paint, tab).any(),
        None => start::page(paint).any(),
    };
    let workspace = column! {
        if let Some(notice) = paint.notice() {
            banner(notice.clone())
                .on_dismiss(tr!("paint.notice.dismiss"), Paint::dismiss_notice);
        }
        container(body).grow(1.0).fill_width();
    }
    .fill()
    .build()
    .keyed(STAGE);
    let mut layers = vec![workspace];
    if let (Some(prompt), Some(tab)) = (paint.unsaved_prompt(), paint.prompted_tab()) {
        layers.extend(
            unsaved_prompt(&tab.name(), prompt.is_saving(), Paint::answer)
                .keyed(("paint-unsaved", prompt.tab()))
                .centered_on(STAGE)
                .with_scrim(),
        );
    }
    layers.extend(new_image::view(paint));
    let shortcuts = shortcuts(
        column_of(layers).fill(),
        Command::pressed,
        Command::enabled,
        Command::run,
    )
    .standard(Command::from_standard)
    .holding(Key::Space, Paint::hold_space);

    drop_zone(shortcuts)
        .accepts(|offer: &DragOffer| offer.offers(URI_LIST).then(|| Accept::copy(URI_LIST)))
        .on_drop(|offer: DragOffer, _: DragAction| Paint::dropped(offer))
        .fill()
}

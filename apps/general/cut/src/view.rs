use rikka_app::prelude::*;
use rikka_ui::{Accept, DragAction, DragOffer};
use rikka_widgets::{column, row};

use crate::command::Command;
use crate::cut::Cut;
use crate::{attributes, export, monitor, panel, reformat, relink, status, timeline};

pub(crate) const URI_LIST: &str = "text/uri-list";
pub(crate) const STAGE: &str = "cut-stage";

pub(crate) fn view(cut: &Cut) -> impl View {
    let ratio = cut.ratio().clone();
    let upper = row! {
        container(monitor::monitor(cut)).grow(1.0).fill_height();
        rule_upright();
        component(|cut: &Cut| cut, panel::panel);
    }
    .fill();
    let studio = split(
        Axis::Vertical,
        cut.ratio().first(),
        upper,
        timeline::timeline(cut),
    )
    .on_resize(move |share: f32| {
        ratio.set(share);
        skip_view();
    })
    .grow(1.0)
    .fill_width();
    let body = column! {
        if let Some(notice) = cut.notice() {
            banner(notice.clone()).on_dismiss(tr!("cut.notice.dismiss"), Cut::dismiss_notice);
        }
        studio;
        status::status(cut);
    }
    .fill();
    let shortcuts = shortcuts(body, Command::pressed, Command::enabled, Command::run)
        .standard(Command::from_standard)
        .paused(cut.is_modal());

    let mut layers = vec![
        drop_zone(shortcuts)
            .accepts(|offer: &DragOffer| offer.offers(URI_LIST).then(|| Accept::copy(URI_LIST)))
            .on_drop(|offer: DragOffer, _: DragAction| Cut::dropped(offer, None))
            .content_first(true)
            .fill()
            .build()
            .keyed(STAGE),
    ];
    layers.extend(export::view(cut));
    if let Some(reformatting) = cut.reformatting() {
        layers.extend(reformat::sheet(reformatting));
    }
    if let Some(relinking) = cut.relinking() {
        layers.extend(relink::sheet(relinking));
    }
    if let Some(pasting) = cut.pasting() {
        layers.extend(attributes::sheet(pasting));
    }
    if let Some(prompt) = cut.unsaved_prompt() {
        layers.extend(
            unsaved_prompt(&cut.name(), prompt.is_saving(), Cut::answer)
                .keyed(("cut-unsaved", 0))
                .centered_on(STAGE)
                .with_scrim(),
        );
    }
    column_of(layers).fill()
}

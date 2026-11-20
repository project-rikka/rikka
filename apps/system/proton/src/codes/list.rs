use std::sync::Arc;

use proton_codes::StoredCode;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::codes::Codes;
use crate::{pane, wording};

const ROW: f32 = 76.0;

pub(crate) fn toolbar(codes: &Codes) -> impl View {
    pane::toolbar(
        row! {
            text_field(codes.search())
                .placeholder(tr!("proton.codes.search"))
                .claims_focus(codes.seeking())
                .on_blur(Codes::sought)
                .control_size(ControlSize::Small)
                .on_change(|codes: &mut Codes| codes.look())
                .width(280.0);
            spacer().grow(1.0);
            button(
                row! {
                    icon(MaterialSymbol::ADD).extent(Length::px(15.0));
                    text(tr!("proton.codes.add")).caption().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .primary()
            .small()
            .on_press(|codes: &mut Codes| codes.write());
            button(icon(MaterialSymbol::SYNC).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .loading(codes.is_syncing())
                .on_press(|codes: &mut Codes| codes.sync());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}

pub(crate) fn view(codes: &Codes) -> Element {
    let filtered = Arc::clone(codes.store().filtered());

    if filtered.is_empty() {
        return match codes.is_syncing() || codes.is_recalling() {
            true => pane::working(tr!("proton.codes.reading")).build(),
            false => pane::nothing(MaterialSymbol::PASSWORD, tr!("proton.codes.empty")).build(),
        };
    }

    let now = codes.now();

    lazy_column(filtered.len(), move |slot| {
        cell(&filtered[slot.index], now).fill_width().height(ROW)
    })
    .uniform(ROW)
    .place(codes.place())
    .grow(1.0)
    .fill_width()
    .build()
}

fn cell(entry: &StoredCode, now: jiff::Timestamp) -> Element {
    let code = entry.code_at(now);
    let id = entry.id().to_owned();
    let editing = id.clone();
    let dropping = id.clone();

    container(
        row! {
            pane::avatar(&wording::initial(entry.title()), palette().accent);
            column! {
                text(entry.title().to_owned()).label().semibold().no_wrap();
                text(match entry.subtitle().is_empty() {
                    true => entry.note().unwrap_or_default().to_owned(),
                    false => entry.subtitle().to_owned(),
                })
                .caption()
                .muted()
                .no_wrap();
            }
            .gap(Space::HAIR)
            .width(200.0);
            spacer().grow(1.0);
            match &code {
                Some(code) => text(code.grouped()).title().build(),
                None => text(tr!("proton.codes.unreadable")).caption().danger().build(),
            }
            if let Some(code) = &code {
                gauge(code.remaining_fraction()).width(46.0);
                text(format!("{}s", code.remaining)).caption().muted().no_wrap();
                button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |codes: &mut Codes| codes.copy(&id));
            }
            button(icon(MaterialSymbol::EDIT).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(move |codes: &mut Codes| codes.edit(&editing));
            button(icon(MaterialSymbol::DELETE).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .on_press(move |codes: &mut Codes| codes.drop_code(&dropping));
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(16.0, 10.0))
    .fill_width()
    .build()
}

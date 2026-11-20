use rikka_thread::QueuedAsk;

use crate::chat::Chat;
use crate::mcp::kept_attachments;
use crate::prelude::*;
use rikka_widgets::row;

const GLYPH: f32 = 15.0;
const TOOL: f32 = 14.0;
const SHOWN: usize = 160;

pub(crate) fn view<C: Chat>(waiting: &[QueuedAsk]) -> Option<Flex> {
    if waiting.is_empty() {
        return None;
    }
    Some(
        column_of(waiting.iter().map(|waiting| queued::<C>(waiting)))
            .gap(Space::HAIR)
            .fill_width(),
    )
}

fn queued<C: Chat>(waiting: &QueuedAsk) -> Element {
    let palette = palette();
    let id = waiting.id();
    let ask = waiting.ask();
    let attached = kept_attachments(ask.text(), ask.sent_text(), ask.attachments()).len();
    let flat = ask.text().trim().replace('\n', " ");
    let preview = rikka_format::truncate(&flat, SHOWN).into_owned();

    container(
        row! {
            icon(MaterialSymbol::SCHEDULE)
                .extent(Length::px(GLYPH))
                .color(palette.text_muted);
            text(preview).caption().muted().no_wrap().grow(1.0).min_width(0.0);
            if attached > 0 {
                row! {
                    icon(MaterialSymbol::ATTACH_FILE)
                        .extent(Length::px(GLYPH))
                        .color(palette.text_muted);
                    text(attached.to_string()).caption().muted();
                }
                .gap(Space::HAIR)
                .align_items(Align::Center);
            }
            if let Some(recall) = C::recall() {
                tool(MaterialSymbol::EDIT).on_press(recall.map(move |()| (id,)));
            }
            if let Some(withdraw) = C::withdraw() {
                tool(MaterialSymbol::CLOSE).on_press(withdraw.map(move |()| (id,)));
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .background(palette.selection)
    .radius(Corner::Control)
    .padding(Inset::edges(4.0, 4.0, 4.0, 10.0))
    .fill_width()
    .transition(fade().enter(Curve::Spring(Spring::DEFAULT_SPATIAL)))
    .keyed(("chat-waiting", id))
}

fn tool(symbol: MaterialSymbol) -> Button {
    button(
        icon(symbol)
            .extent(Length::px(TOOL))
            .color(palette().text_muted),
    )
    .icon_only()
    .ghost()
    .small()
}

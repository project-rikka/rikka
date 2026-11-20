use rikka_llm::{Media, ModelInfo};
use rikka_thread::Attachment;

use crate::chat::Chat;
use crate::prelude::*;
use rikka_widgets::{column, row};

const THUMB: f32 = 60.0;
const NARROWEST: f32 = 40.0;
const WIDEST: f32 = 120.0;
const CARD: f32 = 180.0;
const GLYPH: f32 = 20.0;
const CROSS: f32 = 14.0;

pub(crate) fn strip<C: Chat>(attachments: &[Attachment]) -> Flex {
    attachment_chips::<C>(attachments, true)
}

pub(crate) fn settled<C: Chat>(attachments: &[Attachment]) -> Flex {
    attachment_chips::<C>(attachments, false)
}

fn attachment_chips<C: Chat>(attachments: &[Attachment], removable: bool) -> Flex {
    row_of(
        attachments
            .iter()
            .enumerate()
            .map(|(at, attached)| chip::<C>(at, attached, removable)),
    )
    .gap(Space::SNUG)
    .align_items(Align::Start)
    .wrap_items()
    .fill_width()
}

fn chip<C: Chat>(at: usize, attached: &Attachment, removable: bool) -> Element {
    let body = match attached.media().is_picture() {
        true => thumbnail(attached).any(),
        false => card(attached).any(),
    };
    match removable {
        true => row! {
            body;
            remove::<C>(at);
        }
        .align_items(Align::Start)
        .any(),
        false => body,
    }
}

fn thumbnail(attached: &Attachment) -> impl View {
    let (width, height) = attached
        .extent()
        .filter(|(width, height)| *width > 0 && *height > 0)
        .unwrap_or((1, 1));
    let across = (THUMB * width as f32 / height as f32).clamp(NARROWEST, WIDEST);
    container(
        image(ImageSource::Bytes(attached.data().clone()))
            .natural_size(width as f32, height as f32)
            .fit(ImageFit::Cover)
            .radius(Corner::Item)
            .fill(),
    )
    .radius(Corner::Item)
    .border(1.0, palette().outline)
    .width(across)
    .height(THUMB)
}

pub(crate) fn unseen(model: &ModelInfo) -> impl View {
    row! {
        icon(MaterialSymbol::VISIBILITY_OFF)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        text(tr!("chat.attachment.unseen", model = model.name()))
            .caption()
            .muted()
            .grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn card(attached: &Attachment) -> impl View {
    let palette = palette();
    container(
        row! {
            icon(symbol(attached.media()))
                .extent(Length::px(GLYPH))
                .color(palette.text_muted);
            column! {
                text(attached.name().to_owned()).caption().medium().no_wrap();
                text(kind(attached.media())).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .surface()
    .radius(Corner::Item)
    .border(1.0, palette.outline)
    .padding(Inset::xy(10.0, 8.0))
    .max_width(CARD)
    .height(THUMB)
}

fn remove<C: Chat>(at: usize) -> impl View {
    button(
        icon(MaterialSymbol::CLOSE)
            .extent(Length::px(CROSS))
            .color(palette().text_muted),
    )
    .icon_only()
    .ghost()
    .small()
    .on_press(C::unattach().map(move |()| (at,)))
}

fn symbol(media: Media) -> MaterialSymbol {
    match media {
        Media::Pdf => MaterialSymbol::PICTURE_AS_PDF,
        _ => MaterialSymbol::DESCRIPTION,
    }
}

fn kind(media: Media) -> String {
    media.extension().to_ascii_uppercase()
}

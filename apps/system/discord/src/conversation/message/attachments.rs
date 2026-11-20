use std::time::Duration;

use discord_core::{Attachment, AttachmentKind, Picture};
use rikka_app::prelude::*;
use rikka_format::{format_bytes, format_duration};
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::chat::Chat;
use crate::conversation::Conversation;
use crate::conversation::lightbox::{Album, Shot};
use crate::conversation::media::{Media, blurred, frames};
use crate::conversation::widgets::animated;
use crate::pictures;

pub(crate) const WIDEST: f32 = 400.0;
pub(crate) const TALLEST: f32 = 300.0;
const CARD: f32 = 420.0;

pub(crate) fn fitted(width: u32, height: u32) -> Size {
    if width == 0 || height == 0 {
        return Size::new(WIDEST, WIDEST * 9.0 / 16.0);
    }
    let (width, height) = (width as f32, height as f32);
    let scale = (WIDEST / width).min(TALLEST / height).min(1.0);
    Size::new((width * scale).max(48.0), (height * scale).max(48.0))
}

pub(crate) fn view(
    conversation: &Conversation,
    attachments: &[Attachment],
    album: &Album,
) -> Vec<Element> {
    attachments
        .iter()
        .map(|attachment| match &attachment.kind {
            AttachmentKind::Image { width, height } => {
                picture(conversation.media(), attachment, *width, *height, album)
            }
            AttachmentKind::Video { width, height } => {
                movie(conversation.media(), attachment, *width, *height)
            }
            AttachmentKind::Audio { millis } => sound(conversation.media(), attachment, *millis),
            AttachmentKind::File { .. } => file(attachment),
        })
        .collect()
}

pub(crate) fn shots(media: &Media, attachments: &[Attachment]) -> Vec<Shot> {
    attachments
        .iter()
        .filter(|attachment| !attachment.spoiler || media.is_revealed(&attachment.id))
        .filter_map(|attachment| match attachment.kind {
            AttachmentKind::Image { width, height } => Some(Shot {
                url: attachment.url.clone(),
                name: attachment.name.clone(),
                full: Picture::fixed(attachment.url.clone()),
                preview: attachment.preview.clone(),
                width,
                height,
                animated: is_animated(attachment, &source_of(attachment)),
                description: attachment.description.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub(crate) fn shown_picture(picture: &Picture, size: Size, animate: bool) -> Element {
    let side = size.width.max(size.height);
    let face = match animate {
        true => frames(picture, side, size).map(|frames| animated(frames, ImageFit::Cover)),
        false => {
            pictures::fetched(picture, side).map(|bytes| image(bytes).fit(ImageFit::Cover).any())
        }
    };
    match face {
        Some(face) => face
            .size(Length::px(size.width), Length::px(size.height))
            .any(),
        None => container(spinner())
            .center()
            .background(palette().surface_variant)
            .size(Length::px(size.width), Length::px(size.height))
            .any(),
    }
}

fn source_of(attachment: &Attachment) -> Picture {
    attachment
        .preview
        .clone()
        .unwrap_or_else(|| Picture::fixed(attachment.url.clone()))
}

fn is_animated(attachment: &Attachment, picture: &Picture) -> bool {
    picture.is_animated()
        || attachment
            .name
            .rsplit('.')
            .next()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
}

fn spoiler_cover(
    media: &Media,
    attachment: &Attachment,
    picture: &Picture,
    size: Size,
) -> Option<Element> {
    if !attachment.spoiler || media.is_revealed(&attachment.id) {
        return None;
    }
    let blur = match blurred(picture) {
        Some(source) => image(source)
            .fit(ImageFit::Cover)
            .size(Length::px(size.width), Length::px(size.height))
            .any(),
        None => container(spacer())
            .background(palette().surface_variant)
            .size(Length::px(size.width), Length::px(size.height))
            .any(),
    };
    let id = attachment.id.clone();
    Some(
        container(column_of([
            blur,
            container(
                container(
                    text(tr!("discord.conversation.spoiler"))
                        .caption()
                        .bold()
                        .color(Color::WHITE),
                )
                .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                .background(Color::BLACK.with_alpha(0.6))
                .radius(Corner::Control),
            )
            .center()
            .size(Length::px(size.width), Length::px(size.height))
            .position(0.0, 0.0)
            .any(),
        ]))
        .radius(Corner::Item)
        .clip_content()
        .focusless()
        .on_press(move |media: &mut Media| media.reveal(&id))
        .size(Length::px(size.width), Length::px(size.height))
        .any(),
    )
}

fn picture(
    media: &Media,
    attachment: &Attachment,
    width: u32,
    height: u32,
    album: &Album,
) -> Element {
    let size = fitted(width, height);
    let picture = source_of(attachment);
    if let Some(cover) = spoiler_cover(media, attachment, &picture, size) {
        return cover;
    }
    album.framed(
        &attachment.url,
        container(shown_picture(
            &picture,
            size,
            is_animated(attachment, &picture),
        ))
        .radius(Corner::Item)
        .clip_content()
        .size(Length::px(size.width), Length::px(size.height)),
    )
}

fn movie(media: &Media, attachment: &Attachment, width: u32, height: u32) -> Element {
    let size = fitted(width, height);
    let playing = media
        .playing()
        .is_some_and(|playing| playing.id == attachment.id && playing.video);
    if let (true, Some(controller)) = (playing, media.video()) {
        let paused = media.is_paused();
        return column_of([
            video(controller)
                .fit(ImageFit::Contain)
                .size(Length::px(size.width), Length::px(size.height))
                .any(),
            container(
                button(
                    icon(match paused {
                        true => MaterialSymbol::PLAY_ARROW,
                        false => MaterialSymbol::PAUSE,
                    })
                    .extent(Length::px(20.0)),
                )
                .icon_only()
                .secondary()
                .small()
                .on_press(|media: &mut Media| media.toggle_pause()),
            )
            .padding(Inset::all(Space::SNUG))
            .position(0.0, size.height - 48.0)
            .any(),
        ])
        .background(Color::BLACK)
        .size(Length::px(size.width), Length::px(size.height))
        .any();
    }
    let poster = match &attachment.preview {
        Some(preview) => shown_picture(preview, size, false),
        None => container(spacer())
            .background(Color::BLACK)
            .size(Length::px(size.width), Length::px(size.height))
            .any(),
    };
    let fetching = media.fetching() == Some(attachment.id.as_str());
    let (id, url, name) = (
        attachment.id.clone(),
        attachment.url.clone(),
        attachment.name.clone(),
    );
    let middle = match fetching {
        true => spinner().any(),
        false => container(
            icon(MaterialSymbol::PLAY_ARROW)
                .extent(Length::px(32.0))
                .color(Color::WHITE),
        )
        .center()
        .background(Color::BLACK.with_alpha(0.55))
        .radius(28.0)
        .size(Length::px(56.0), Length::px(56.0))
        .any(),
    };
    container(column_of([
        poster,
        container(middle)
            .center()
            .size(Length::px(size.width), Length::px(size.height))
            .position(0.0, 0.0)
            .any(),
    ]))
    .radius(Corner::Item)
    .clip_content()
    .focusless()
    .on_press(move |chat: &mut Chat| chat.play(&id, &url, &name, true))
    .size(Length::px(size.width), Length::px(size.height))
    .any()
}
fn sound(media: &Media, attachment: &Attachment, millis: Option<u64>) -> Element {
    let playing = media
        .playing()
        .is_some_and(|playing| playing.id == attachment.id && !playing.video);
    let (elapsed, length) = match playing {
        true => media.progress(),
        false => (Duration::ZERO, millis.map(Duration::from_millis)),
    };
    let length = length.or(millis.map(Duration::from_millis));
    let share = length
        .filter(|length| !length.is_zero())
        .map_or(0.0, |length| elapsed.as_secs_f32() / length.as_secs_f32());
    let fetching = media.fetching() == Some(attachment.id.as_str());
    let symbol = match (playing, media.is_paused()) {
        (true, false) => MaterialSymbol::PAUSE,
        _ => MaterialSymbol::PLAY_ARROW,
    };
    let (id, url, name) = (
        attachment.id.clone(),
        attachment.url.clone(),
        attachment.name.clone(),
    );
    let (saved_url, saved_name) = (attachment.url.clone(), attachment.name.clone());
    container(
        column! {
            row! {
                icon(MaterialSymbol::AUDIO_FILE).extent(Length::px(28.0)).color(palette().accent);
                column! {
                    text(attachment.name.clone()).label().semibold().no_wrap().fill_width();
                    text(format_bytes(attachment.size)).caption().muted().no_wrap();
                }
                .gap(Space::HAIR)
                .grow(1.0)
                .min_width(0.0);
                button(icon(MaterialSymbol::DOWNLOAD).extent(Length::px(18.0)))
                    .icon_only()
                    .ghost()
                    .small()
                    .on_press(move |chat: &mut Chat| chat.download(&saved_url, &saved_name));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            row! {
                match fetching {
                    true => spinner().any(),
                    false => button(icon(symbol).extent(Length::px(20.0)))
                        .icon_only()
                        .secondary()
                        .small()
                        .on_press(move |chat: &mut Chat| chat.play(&id, &url, &name, false))
                        .any(),
                };
                progress(share).grow(1.0);
                text(format!(
                    "{} / {}",
                    format_duration(elapsed),
                    length.map(format_duration).unwrap_or_else(|| "--:--".to_owned())
                ))
                .caption()
                .muted()
                .no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .padding(Inset::all(Space::NORMAL))
    .background(palette().surface)
    .border(1.0, palette().outline)
    .radius(Corner::Item)
    .width(CARD)
    .any()
}

fn file(attachment: &Attachment) -> Element {
    let (url, name) = (attachment.url.clone(), attachment.name.clone());
    container(
        row! {
            icon(MaterialSymbol::DESCRIPTION).extent(Length::px(32.0)).color(palette().accent);
            column! {
                text(attachment.name.clone()).label().semibold().accent().no_wrap().fill_width();
                text(format_bytes(attachment.size)).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            button(icon(MaterialSymbol::DOWNLOAD).extent(Length::px(20.0)))
                .icon_only()
                .ghost()
                .on_press(move |chat: &mut Chat| chat.download(&url, &name));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::all(Space::NORMAL))
    .background(palette().surface)
    .border(1.0, palette().outline)
    .radius(Corner::Item)
    .width(CARD)
    .any()
}

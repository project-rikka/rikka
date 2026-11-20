use discord_core::{Embed, EmbedKind, EmbedLine, EmbedMedia};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use super::attachments::{fitted, shown_picture};
use super::rich::{Reveal, Shaped, Shaping, render};
use crate::conversation::lightbox::{Album, Shot};
use crate::conversation::media::open_link;
use crate::conversation::time;
use crate::widgets::picture_tile;

const WIDEST: f32 = 520.0;
const THUMBNAIL: f32 = 80.0;
const ACROSS: usize = 3;

pub(crate) struct Prepared {
    embed: Embed,
    description: Vec<Shaped>,
    fields: Vec<(Vec<Shaped>, Vec<Shaped>, bool)>,
}

pub(crate) fn prepare(shaping: &mut Shaping<'_>, embed: &Embed) -> Prepared {
    Prepared {
        embed: embed.clone(),
        description: shaping.rich(&embed.description),
        fields: embed
            .fields
            .iter()
            .map(|field| {
                (
                    shaping.rich(&field.name),
                    shaping.rich(&field.value),
                    field.inline,
                )
            })
            .collect(),
    }
}

pub(crate) fn render_embed(prepared: &Prepared, reveal: &Reveal, album: &Album) -> Element {
    let embed = &prepared.embed;
    if let Some(media) = bare_media(embed) {
        return media_view(media, embed.kind == EmbedKind::Gif, album);
    }
    let bar = embed.color.map_or(palette().outline, Color::hex);
    let mut lines: Vec<Element> = Vec::new();
    if let Some(provider) = &embed.provider {
        lines.push(text(provider.clone()).caption().muted().no_wrap().any());
    }
    if let Some(author) = &embed.author {
        lines.push(line_of(author, 24.0, true));
    }
    if let Some(title) = &embed.title {
        let shown = text(title.clone()).label().semibold();
        lines.push(match &embed.url {
            Some(url) => {
                let url = url.clone();
                container(shown.accent())
                    .focusless()
                    .on_press(move || {
                        open_link(&url);
                        skip_view();
                    })
                    .any()
            }
            None => shown.any(),
        });
    }
    if !prepared.description.is_empty() {
        lines.push(render(&prepared.description, reveal));
    }
    if !prepared.fields.is_empty() {
        lines.push(fields(&prepared.fields, reveal));
    }
    if let Some(image) = &embed.image {
        lines.push(media_view(image, false, album));
    }
    if let Some(video) = &embed.video
        && embed.image.is_none()
    {
        let poster = embed.thumbnail.as_ref().unwrap_or(video);
        lines.push(played(poster, &video.url));
    }
    if embed.footer.is_some() || embed.timestamp.is_some() {
        let stamp = embed.timestamp.map(time::sent_at);
        lines.push(
            row! {
                if let Some(footer) = &embed.footer {
                    line_of(footer, 16.0, false);
                }
                if let Some(stamp) = stamp {
                    text(format!("• {stamp}")).caption().muted().no_wrap();
                }
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .any(),
        );
    }
    let thumbnail = match (&embed.thumbnail, &embed.video) {
        (Some(thumbnail), None) => Some(
            album.framed(
                &thumbnail.url,
                container(picture_tile(
                    Some(&thumbnail.picture),
                    "",
                    THUMBNAIL,
                    Corner::Item.radius(),
                ))
                .size(Length::px(THUMBNAIL), Length::px(THUMBNAIL)),
            ),
        ),
        _ => None,
    };
    let content = row! {
        column_of(lines).gap(Space::TIGHT).grow(1.0).min_width(0.0);
        if let Some(thumbnail) = thumbnail {
            thumbnail;
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Start)
    .fill_width();
    container(
        row! {
            container(spacer()).background(bar).width(4.0).fill_height();
            container(content)
                .padding(Inset::all(Space::NORMAL))
                .grow(1.0)
                .min_width(0.0);
        }
        .fill_width(),
    )
    .background(palette().surface)
    .radius(Corner::Item)
    .clip_content()
    .max_width(WIDEST)
    .any()
}

fn line_of(line: &EmbedLine, side: f32, bold: bool) -> Element {
    let caption = text(line.text.clone()).caption().no_wrap().min_width(0.0);
    let caption = match bold {
        true => caption.semibold().any(),
        false => caption.muted().any(),
    };
    let body = row! {
        if let Some(icon) = &line.icon {
            picture_tile(Some(icon), "", side, side / 2.0);
        }
        caption;
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center);
    match &line.url {
        Some(url) => {
            let url = url.clone();
            container(body)
                .focusless()
                .on_press(move || {
                    open_link(&url);
                    skip_view();
                })
                .any()
        }
        None => body.any(),
    }
}

fn fields(fields: &[(Vec<Shaped>, Vec<Shaped>, bool)], reveal: &Reveal) -> Element {
    let mut rows: Vec<Element> = Vec::new();
    let mut inline: Vec<Element> = Vec::new();
    let flush = |inline: &mut Vec<Element>, rows: &mut Vec<Element>| {
        if !inline.is_empty() {
            rows.push(
                row_of(std::mem::take(inline))
                    .gap(Space::NORMAL)
                    .align_items(Align::Start)
                    .fill_width()
                    .any(),
            );
        }
    };
    for (name, value, inlined) in fields {
        let field = column! {
            render(name, reveal);
            render(value, reveal);
        }
        .gap(Space::HAIR);
        match inlined {
            true => {
                inline.push(field.grow(1.0).min_width(0.0).any());
                if inline.len() == ACROSS {
                    flush(&mut inline, &mut rows);
                }
            }
            false => {
                flush(&mut inline, &mut rows);
                rows.push(field.fill_width().any());
            }
        }
    }
    flush(&mut inline, &mut rows);
    column_of(rows).gap(Space::SNUG).fill_width().any()
}

pub(super) fn shots(prepared: &Prepared) -> Vec<Shot> {
    let embed = &prepared.embed;
    if let Some(media) = bare_media(embed) {
        return vec![shot_of(media, embed.kind == EmbedKind::Gif)];
    }
    let thumbnail = match (&embed.thumbnail, &embed.video) {
        (Some(thumbnail), None) => Some(thumbnail),
        _ => None,
    };
    embed
        .image
        .iter()
        .chain(thumbnail)
        .map(|media| shot_of(media, false))
        .collect()
}

fn bare_media(embed: &Embed) -> Option<&EmbedMedia> {
    let bare = matches!(embed.kind, EmbedKind::Image | EmbedKind::Gif)
        && embed.title.is_none()
        && embed.description.is_empty();
    bare.then(|| embed.image.as_ref().or(embed.thumbnail.as_ref()))
        .flatten()
}

fn shot_of(media: &EmbedMedia, animate: bool) -> Shot {
    Shot {
        url: media.url.clone(),
        name: media
            .url
            .rsplit('/')
            .next()
            .and_then(|last| last.split('?').next())
            .unwrap_or_default()
            .to_owned(),
        full: media.picture.clone(),
        preview: Some(media.picture.clone()),
        width: media.width,
        height: media.height,
        animated: animate || media.picture.is_animated(),
        description: None,
    }
}

fn media_view(media: &EmbedMedia, animate: bool, album: &Album) -> Element {
    let size = fitted(media.width, media.height);
    album.framed(
        &media.url,
        container(shown_picture(
            &media.picture,
            size,
            animate || media.picture.is_animated(),
        ))
        .radius(Corner::Item)
        .clip_content()
        .size(Length::px(size.width), Length::px(size.height)),
    )
}

fn played(poster: &EmbedMedia, url: &str) -> Element {
    let size = fitted(poster.width, poster.height);
    let url = url.to_owned();
    container(column_of([
        shown_picture(&poster.picture, size, false),
        container(
            container(
                icon(MaterialSymbol::PLAY_ARROW)
                    .extent(Length::px(32.0))
                    .color(Color::WHITE),
            )
            .center()
            .background(Color::BLACK.with_alpha(0.55))
            .radius(28.0)
            .size(Length::px(56.0), Length::px(56.0)),
        )
        .center()
        .size(Length::px(size.width), Length::px(size.height))
        .position(0.0, 0.0)
        .any(),
    ]))
    .radius(Corner::Item)
    .clip_content()
    .focusless()
    .on_press(move || {
        open_link(&url);
        skip_view();
    })
    .size(Length::px(size.width), Length::px(size.height))
    .any()
}

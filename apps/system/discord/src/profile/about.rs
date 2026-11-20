use discord_core::{ActivityKind, Connected, Role};
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::{column, row};

use super::elapsed::elapsed;
use super::look::Look;
use super::{Profiles, Tab};
use crate::brand;
use crate::chat::Chat;
use crate::conversation::{open_link, rich_view, time};
use crate::pictures;
use crate::widgets::{mark_of, picture_tile};

const MARK: f32 = 16.0;
const ART: f32 = 64.0;
const DOT: f32 = 10.0;
const NOTE_MOST: usize = 256;

pub(super) fn section(title: Label, body: impl View) -> Element {
    column! {
        text(title).caption().bold().muted().no_wrap();
        body;
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

pub(super) fn basics(look: &Look<'_>) -> Vec<Element> {
    let mut shown: Vec<Element> = bio(look).into_iter().chain(activity(look)).collect();
    shown.push(member_since(look));
    shown
}

fn bio(look: &Look<'_>) -> Option<Element> {
    Some(rich_view(
        look.linked,
        look.target.space.as_ref(),
        look.bio()?,
        mark_of(("discord-bio", look.mark())),
        look.animate,
    ))
}

fn member_since(look: &Look<'_>) -> Element {
    let joined = look.joined.zip(look.space.as_ref());
    section(
        tr!("discord.profile.member-since"),
        row! {
            icon(brand::mark()).extent(Length::px(MARK)).color(brand::blurple());
            text(time::day_label(look.created())).label().no_wrap();
            if let Some((joined, space)) = joined {
                text("•").label().muted().no_wrap();
                picture_tile(space.icon.as_ref(), &space.name, MARK, Corner::Detail.radius());
                text(time::day_label(joined)).label().no_wrap();
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .wrap_items()
        .fill_width(),
    )
}

pub(super) fn roles(look: &Look<'_>, limit: Option<(usize, Handler<()>)>) -> Option<Element> {
    if look.roles.is_empty() {
        return None;
    }
    let most = limit.as_ref().map_or(look.roles.len(), |(most, _)| *most);
    let mut chips: Vec<Element> = look.roles.iter().take(most).map(chip).collect();
    let hidden = look.roles.len().saturating_sub(most);
    if let Some((_, more)) = limit.filter(|_| hidden > 0) {
        chips.push(
            container(text(format!("+{hidden}")).caption().semibold().no_wrap())
                .padding(Inset::xy(Space::SNUG, Space::HAIR))
                .background(palette().surface_variant)
                .radius(Corner::Detail)
                .focusless()
                .on_press(more)
                .any(),
        );
    }
    Some(section(
        tr!("discord.profile.roles"),
        row_of(chips).gap(Space::TIGHT).wrap_items().fill_width(),
    ))
}

fn chip(role: &Role) -> Element {
    let color = role.color.map_or(palette().text_muted, Color::hex);
    container(
        row! {
            container(spacer())
                .background(color)
                .radius(DOT / 2.0)
                .size(Length::px(DOT), Length::px(DOT));
            if let Some(face) = role.icon.as_ref().and_then(|icon| pictures::fetched(icon, MARK)) {
                image(face).fit(ImageFit::Contain).size(Length::px(MARK), Length::px(MARK));
            }
            text(role.name.clone()).caption().medium().no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::SNUG, Space::HAIR))
    .background(palette().surface_variant)
    .radius(Corner::Detail)
    .any()
}

fn activity(look: &Look<'_>) -> Option<Element> {
    let activity = look.presence.activity.as_ref()?;
    let (title, lead) = match activity.kind {
        ActivityKind::Playing => (
            tr!("discord.profile.activity.playing"),
            Some(activity.name.clone()),
        ),
        ActivityKind::Streaming => (
            tr!(
                "discord.profile.activity.streaming",
                name = activity.name.as_str()
            ),
            None,
        ),
        ActivityKind::Listening => (
            tr!(
                "discord.profile.activity.listening",
                name = activity.name.as_str()
            ),
            None,
        ),
        ActivityKind::Watching => (
            tr!(
                "discord.profile.activity.watching",
                name = activity.name.as_str()
            ),
            None,
        ),
        ActivityKind::Competing => (
            tr!(
                "discord.profile.activity.competing",
                name = activity.name.as_str()
            ),
            None,
        ),
    };
    let lines: Vec<String> = lead
        .into_iter()
        .chain(activity.details.clone())
        .chain(activity.state.clone())
        .collect();
    let art = activity
        .art
        .as_ref()
        .and_then(|art| pictures::fetched(art, ART));
    Some(section(
        title,
        container(
            row! {
                if let Some(art) = art {
                    image(art)
                        .fit(ImageFit::Cover)
                        .radius(Corner::Item)
                        .size(Length::px(ART), Length::px(ART));
                }
                column! {
                    for (at, line) in lines.into_iter().enumerate() {
                        match at {
                            0 => text(line).label().semibold().no_wrap().any(),
                            _ => text(line).caption().no_wrap().any(),
                        };
                    }
                    if let Some(started) = activity.started {
                        row! {
                            icon(MaterialSymbol::SCHEDULE).extent(Length::px(14.0)).color(palette().success);
                            elapsed(started).grow(1.0).min_width(0.0);
                        }
                        .gap(Space::TIGHT)
                        .align_items(Align::Center)
                        .fill_width();
                    }
                }
                .gap(Space::HAIR)
                .grow(1.0)
                .min_width(0.0);
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width(),
        )
        .padding(Inset::all(Space::NORMAL))
        .background(palette().surface_variant)
        .radius(Corner::Item)
        .fill_width(),
    ))
}

pub(super) fn mutuals(look: &Look<'_>) -> Option<Element> {
    if look.me {
        return None;
    }
    let (friends, spaces) = (look.mutual_friends().len(), look.mutual_spaces().len());
    let mut links: Vec<Element> = Vec::new();
    if friends > 0 {
        links.push(mutual_link(
            look,
            tr!("discord.profile.mutual-friends", count = friends as i64),
            Tab::Friends,
        ));
    }
    if spaces > 0 {
        if !links.is_empty() {
            links.push(text("•").caption().muted().no_wrap().any());
        }
        links.push(mutual_link(
            look,
            tr!("discord.profile.mutual-servers", count = spaces as i64),
            Tab::Servers,
        ));
    }
    (!links.is_empty()).then(|| {
        row_of(links)
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .wrap_items()
            .fill_width()
            .any()
    })
}

fn mutual_link(look: &Look<'_>, caption: Label, tab: Tab) -> Element {
    let target = look.target.clone();
    container(text(caption).caption().semibold().muted().no_wrap())
        .focusless()
        .on_press(move |chat: &mut Chat| chat.open_full_profile(&target, tab))
        .any()
}

pub(super) fn connections(look: &Look<'_>) -> Option<Element> {
    let connections = &look.profile.as_ref()?.connections;
    if connections.is_empty() {
        return None;
    }
    Some(section(
        tr!("discord.profile.connections"),
        grid_of(2, connections.iter().map(connection))
            .gap(Space::SNUG)
            .fill_width(),
    ))
}

fn connection(connected: &Connected) -> Element {
    let url = connected.url.clone();
    let mut body = container(
        row! {
            icon(platform(&connected.kind)).extent(Length::px(22.0));
            text(connected.name.clone()).label().semibold().no_wrap().grow(1.0).min_width(0.0);
            if connected.verified {
                icon(MaterialSymbol::VERIFIED).extent(Length::px(16.0)).color(palette().accent);
            }
            if url.is_some() {
                icon(MaterialSymbol::OPEN_IN_NEW).extent(Length::px(16.0)).color(palette().text_muted);
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .border(1.0, palette().outline)
    .radius(Corner::Item)
    .focusless()
    .fill_width();
    if let Some(url) = url {
        body = body.on_press(move || {
            open_link(&url);
            skip_view();
        });
    }
    body.any()
}

fn platform(kind: &str) -> IconSource {
    let brand = match kind {
        "github" => BrandSymbol::GITHUB,
        "twitch" => BrandSymbol::TWITCH,
        "youtube" => BrandSymbol::YOUTUBE,
        "steam" => BrandSymbol::STEAM,
        "reddit" => BrandSymbol::REDDIT,
        "spotify" => BrandSymbol::SPOTIFY,
        "twitter" => BrandSymbol::X_TWITTER,
        "bluesky" => BrandSymbol::BLUESKY,
        "tiktok" => BrandSymbol::TIKTOK,
        "instagram" => BrandSymbol::INSTAGRAM,
        "facebook" => BrandSymbol::FACEBOOK,
        "playstation" => BrandSymbol::PLAYSTATION,
        "xbox" => BrandSymbol::XBOX,
        "battlenet" => BrandSymbol::BATTLE_NET,
        "paypal" => BrandSymbol::PAYPAL,
        "mastodon" => BrandSymbol::MASTODON,
        "ebay" => BrandSymbol::EBAY,
        "domain" => return MaterialSymbol::LANGUAGE.into(),
        _ => return MaterialSymbol::LINK.into(),
    };
    brand.into()
}

pub(super) fn note(profiles: &Profiles) -> Element {
    section(
        tr!("discord.profile.note"),
        text_field(profiles.note())
            .placeholder(tr!("discord.profile.note-placeholder"))
            .enabled(profiles.note_loaded())
            .max_len(NOTE_MOST)
            .on_submit(|chat: &mut Chat| chat.save_note())
            .on_blur(|chat: &mut Chat| chat.save_note())
            .fill_width(),
    )
}

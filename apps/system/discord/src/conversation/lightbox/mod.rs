mod actions;
mod chrome;
mod copying;
mod face;
mod framing;
mod stage;

use std::sync::Arc;

use discord_core::Picture;
use rikka_app::prelude::*;
use rikka_ui::{Action, Element, Handler};
use rikka_widgets::{column, row};

use crate::chat::Chat;
use crate::conversation::media::{notice_bar, open_link};
use crate::conversation::widgets::context_row;
use crate::conversation::{Overlay, time};
use crate::widgets::{avatar, picture_tile, with_tooltip};
pub(crate) use actions::picture_items;
use chrome::{CHIP, backdrop, chip, hidden_while, tapped};
use copying::copy_picture;
use face::{Face, look};
use stage::{Chrome, Turn, stage};

const FACE: f32 = 36.0;
const THUMB: f32 = 52.0;
const SAYING: f32 = 720.0;
const NOTICE: f32 = 560.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shot {
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) full: Picture,
    pub(crate) preview: Option<Picture>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) animated: bool,
    pub(crate) description: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sender {
    pub(crate) name: String,
    pub(crate) avatar: Option<Picture>,
    pub(crate) at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Album {
    sender: Arc<Sender>,
    shots: Arc<[Shot]>,
}

impl Album {
    pub(crate) fn new(sender: Sender, shots: Vec<Shot>) -> Album {
        Album {
            sender: Arc::new(sender),
            shots: shots.into(),
        }
    }

    fn opened_at(&self, url: &str) -> Gallery {
        Gallery {
            album: self.clone(),
            at: self
                .shots
                .iter()
                .position(|shot| shot.url == url)
                .unwrap_or_default(),
        }
    }

    pub(crate) fn framed(&self, url: &str, face: Container) -> Element {
        let gallery = self.opened_at(url);
        let pressable = face
            .focusless()
            .on_press(move |overlay: &mut Overlay| overlay.open_gallery(gallery.clone()));
        let Some(shot) = self.shots.iter().find(|shot| shot.url == url).cloned() else {
            return pressable.any();
        };
        context_row(
            Handler::new(move |overlay: &mut Overlay, _: Modifiers| {
                overlay.set_menu_picture(shot.clone())
            }),
            pressable,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Gallery {
    album: Album,
    at: usize,
}

impl Gallery {
    pub(crate) fn showing(&self, at: usize) -> Option<Gallery> {
        (at < self.album.shots.len()).then(|| Gallery {
            album: self.album.clone(),
            at,
        })
    }

    pub(crate) fn stepped(&self, forward: bool) -> Option<Gallery> {
        match forward {
            true => self.showing(self.at + 1),
            false => self.at.checked_sub(1).and_then(|at| self.showing(at)),
        }
    }

    fn shot(&self) -> &Shot {
        &self.album.shots[self.at]
    }

    fn count(&self) -> usize {
        self.album.shots.len()
    }
}

pub(crate) fn view(chat: &Chat) -> Element {
    let conversation = chat.conversation();
    let overlay = conversation.overlay();
    let Some(gallery) = overlay.gallery() else {
        return Element::empty();
    };
    let zoomed = overlay.zoomed();
    let shot = gallery.shot();
    let look = look(shot);
    let over = match look.face {
        Face::Waiting => Some(spinner().any()),
        Face::Failed => Some(failure(shot)),
        Face::Still(_) | Face::Moving(_) => None,
    };
    let notice = conversation.media().notice().map(|notice| {
        container(notice_bar(notice))
            .radius(Corner::Item)
            .clip_content()
            .max_width(NOTICE)
            .fill_width()
            .any()
    });
    let shot_for_menu = shot.clone();
    let copied = shot.full.clone();
    let picture = stage(look.face, look.natural)
        .with_underlay(look.under)
        .chrome(Chrome {
            top: hidden_while(zoomed, top_bar(gallery)).fill_width(),
            middle: middle(gallery, over, zoomed),
            bottom: footer(gallery, notice, zoomed),
            gutter: gutter(gallery.count() > 1),
        })
        .menu(move || picture_items(&shot_for_menu))
        .on_zoom(Action::new(|overlay: &mut Overlay, zoomed: bool| {
            overlay.zoom(zoomed)
        }))
        .on_copy(Action::new(move || {
            copy_picture(copied.clone());
            skip_view();
        }))
        .on_dismiss(Action::new(|overlay: &mut Overlay| overlay.close()))
        .on_turn(Action::new(|overlay: &mut Overlay, turn: Turn| {
            overlay.step_gallery(turn == Turn::Forward)
        }))
        .build()
        .keyed(("discord-lightbox-stage", gallery.at, shot.url.clone()))
        .fill();
    backdrop(
        picture,
        Action::new(|overlay: &mut Overlay| overlay.close()),
    )
}

fn footer(gallery: &Gallery, notice: Option<Element>, zoomed: bool) -> Element {
    let about = column! {
        if let Some(description) = gallery.shot().description.clone() {
            text(description)
                .caption()
                .muted()
                .align(TextAlign::Center)
                .max_width(SAYING);
        }
        if gallery.count() > 1 {
            strip(gallery);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center);
    column! {
        hidden_while(zoomed, about).fill_width();
        if let Some(notice) = notice {
            notice;
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::NONE,
        Space::NORMAL,
        Space::NORMAL,
        Space::NORMAL,
    ))
    .fill_width()
    .any()
}

fn gutter(many: bool) -> f32 {
    match many {
        true => Space::NORMAL.px() + CHIP + Space::SNUG.px(),
        false => Space::NORMAL.px(),
    }
}

fn top_bar(gallery: &Gallery) -> Element {
    let sender = &gallery.album.sender;
    let shot = gallery.shot();
    let when = match gallery.count() {
        1 => time::sent_at(sender.at),
        count => format!(
            "{} • {}",
            time::sent_at(sender.at),
            tr!(
                "discord.conversation.shot-of",
                at = (gallery.at + 1) as i64,
                count = count as i64
            )
        ),
    };
    let copied = shot.full.clone();
    let url_to_open = shot.url.clone();
    let saved = (shot.url.clone(), shot.name.clone());
    row! {
        avatar(sender.avatar.as_ref(), &sender.name, FACE);
        column! {
            text(sender.name.clone()).label().semibold().no_wrap();
            text(when).caption().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        with_tooltip(
            "discord-lightbox-copy",
            chip(
                MaterialSymbol::CONTENT_COPY,
                Some(Handler::new(move || {
                    copy_picture(copied.clone());
                    skip_view();
                })),
            ),
            tr!("discord.conversation.copy-image"),
        );
        with_tooltip(
            "discord-lightbox-open",
            chip(
                MaterialSymbol::OPEN_IN_NEW,
                Some(Handler::new(move || {
                    open_link(&url_to_open);
                    skip_view();
                })),
            ),
            tr!("discord.conversation.open-original"),
        );
        with_tooltip(
            "discord-lightbox-save",
            chip(
                MaterialSymbol::DOWNLOAD,
                Some(Handler::new(move |chat: &mut Chat| chat.save_as(&saved.0, &saved.1))),
            ),
            tr!("discord.conversation.save"),
        );
        with_tooltip(
            "discord-lightbox-close",
            chip(
                MaterialSymbol::CLOSE,
                Some(Handler::new(|overlay: &mut Overlay| overlay.close())),
            ),
            tr!("discord.conversation.close"),
        );
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::all(Space::NORMAL))
    .fill_width()
    .any()
}

fn middle(gallery: &Gallery, over: Option<Element>, zoomed: bool) -> Element {
    let many = gallery.count() > 1;
    let step = |forward: bool| {
        let symbol = match forward {
            true => MaterialSymbol::CHEVRON_RIGHT,
            false => MaterialSymbol::CHEVRON_LEFT,
        };
        let press = gallery
            .stepped(forward)
            .map(|_| Handler::new(move |overlay: &mut Overlay| overlay.step_gallery(forward)));
        let (subject, tip) = match forward {
            true => (
                "discord-lightbox-next",
                tr!("discord.conversation.next-picture"),
            ),
            false => (
                "discord-lightbox-back",
                tr!("discord.conversation.previous-picture"),
            ),
        };
        hidden_while(zoomed, with_tooltip(subject, chip(symbol, press), tip)).width(CHIP)
    };
    row! {
        if many {
            step(false);
        }
        container(over).center().grow(1.0).fill_height().min_width(0.0);
        if many {
            step(true);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NORMAL, Space::NONE))
    .grow(1.0)
    .fill_width()
    .min_height(0.0)
    .any()
}

fn failure(shot: &Shot) -> Element {
    let url = shot.url.clone();
    column! {
        icon(MaterialSymbol::BROKEN_IMAGE).extent(Length::px(48.0)).color(palette().text_muted);
        text(tr!("discord.conversation.picture-failed")).label();
        button(text(tr!("discord.conversation.open-original")))
            .secondary()
            .on_press(move || {
                open_link(&url);
                skip_view();
            });
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .any()
}

fn strip(gallery: &Gallery) -> Element {
    let tiles = gallery.album.shots.iter().enumerate().map(|(index, shot)| {
        let chosen = index == gallery.at;
        let tile = container(picture_tile(
            shot.preview.as_ref().or(Some(&shot.full)),
            &shot.name,
            THUMB,
            Corner::Item.radius(),
        ))
        .padding(Inset::all(Space::HAIR))
        .border(
            2.0,
            match chosen {
                true => palette().text,
                false => Color::TRANSPARENT,
            },
        )
        .radius(Corner::Control)
        .opacity(match chosen {
            true => 1.0,
            false => 0.55,
        });
        tapped(
            tile,
            Handler::new(move |overlay: &mut Overlay| overlay.show_shot(index)),
        )
        .keyed(("discord-lightbox-thumb", index))
    });
    row_of(tiles).gap(Space::SNUG).any()
}

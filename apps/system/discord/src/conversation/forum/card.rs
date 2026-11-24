use discord_core::{
    AttachmentKind, ChannelId, ForumPost, ForumSettings, ForumTag, Message, Picture, PostSort,
    Reaction,
};
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::{column, row};

use super::tags;
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::message::reaction_chip;
use crate::conversation::time;
use crate::pictures;

const EXCERPT: usize = 240;
const THUMBNAIL: f32 = 88.0;
const COVER: f32 = 168.0;
const FOOT_ICON: f32 = 16.0;

#[derive(Clone, Debug, PartialEq)]
struct Author {
    name: String,
    color: Option<Color>,
}

#[derive(Clone, Debug, PartialEq)]
enum Cover {
    Picture(Picture),
    Spoiler,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Card {
    thread: ChannelId,
    name: String,
    pinned: bool,
    locked: bool,
    tags: Vec<ForumTag>,
    author: Option<Author>,
    excerpt: Option<String>,
    cover: Option<Cover>,
    replies: u32,
    at: Option<i64>,
    badge: Option<Reaction>,
    reacting: bool,
}

impl Card {
    pub(super) fn new(
        linked: &LinkedAccount,
        settings: &ForumSettings,
        post: &ForumPost,
        sort: PostSort,
        reacting: bool,
    ) -> Card {
        let thread = linked.channel(&post.thread.id).unwrap_or(&post.thread);
        let about = thread.thread.as_ref();
        let applied = about
            .map(|about| about.applied_tags.as_slice())
            .unwrap_or_default();
        let space = thread.space;
        let starter = post.starter.as_ref();
        Card {
            thread: thread.id,
            name: thread.name.clone(),
            pinned: about.is_some_and(|about| about.pinned),
            locked: about.is_some_and(|about| about.locked),
            tags: settings
                .tags
                .iter()
                .filter(|tag| applied.contains(&tag.id))
                .cloned()
                .collect(),
            author: starter.map(|starter| Author {
                name: linked.display_name(space.as_ref(), &starter.author),
                color: linked
                    .color_of(space.as_ref(), &starter.author.id)
                    .map(Color::hex),
            }),
            excerpt: starter.map(excerpt_of),
            cover: starter.and_then(cover_of),
            replies: about.map_or(post.replies, |about| about.messages),
            at: match sort {
                PostSort::Activity => thread.last_activity,
                PostSort::Created => about.and_then(|about| about.created),
            },
            badge: starter
                .and_then(|starter| {
                    starter
                        .reactions
                        .iter()
                        .fold(None, |best: Option<&Reaction>, reaction| match best {
                            Some(best) if best.count >= reaction.count => Some(best),
                            _ => Some(reaction),
                        })
                        .cloned()
                })
                .or_else(|| {
                    settings.reaction.clone().map(|emoji| Reaction {
                        emoji,
                        count: 0,
                        me: false,
                    })
                }),
            reacting: reacting && starter.is_some(),
        }
    }

    pub(super) fn name(&self) -> u64 {
        self.thread.get()
    }

    pub(super) fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub(super) fn row(&self) -> Element {
        let body = row! {
            column! {
                if !self.tags.is_empty() {
                    tags::chips(&self.tags);
                }
                self.title();
                self.preview();
                self.foot();
            }
            .gap(Space::TIGHT)
            .grow(1.0)
            .min_width(0.0);
            if let Some(cover) = &self.cover {
                container(cover_view(cover, Length::px(THUMBNAIL), Length::px(THUMBNAIL)))
                    .radius(Corner::Control)
                    .clip_content();
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .padding(Inset::all(Space::NORMAL))
        .fill_width();
        self.framed(body.any())
    }

    pub(super) fn tile(&self) -> Element {
        let top = match (&self.cover, &self.excerpt) {
            (Some(cover), _) => cover_view(cover, Length::FILL, Length::px(COVER)),
            (None, Some(excerpt)) if !excerpt.is_empty() => {
                container(text(excerpt.clone()).body().muted().fill_width())
                    .padding(Inset::all(Space::NORMAL))
                    .background(palette().surface_variant)
                    .clip_content()
                    .height(COVER)
                    .fill_width()
                    .any()
            }
            _ => container(
                icon(MaterialSymbol::FORUM)
                    .extent(Length::px(COVER / 4.0))
                    .color(palette().text_muted),
            )
            .center()
            .background(palette().surface_variant)
            .height(COVER)
            .fill_width()
            .any(),
        };
        let body = column! {
            top;
            column! {
                if !self.tags.is_empty() {
                    tags::chips(&self.tags);
                }
                self.title();
                self.foot();
            }
            .gap(Space::TIGHT)
            .padding(Inset::all(Space::NORMAL))
            .fill_width();
        }
        .fill_width();
        self.framed(body.any())
    }

    fn framed(&self, body: Element) -> Element {
        let thread = self.thread;
        local_state(false, move |hovered: bool, set: Setter<bool>| {
            let palette = palette();
            let (fill, edge) = match hovered {
                true => (palette.surface_variant, palette.text_muted.with_alpha(0.35)),
                false => (palette.surface, palette.outline),
            };
            container(body)
                .background(fill)
                .border(1.0, edge)
                .radius(Corner::Panel)
                .clip_content()
                .focusless()
                .on_hover(move |over: bool| {
                    set(over);
                    skip_view();
                })
                .on_press(move |chat: &mut Chat| chat.open_post(&thread))
                .fill_width()
        })
    }

    fn title(&self) -> Element {
        row! {
            if self.pinned {
                icon(MaterialSymbol::PUSH_PIN).extent(Length::px(FOOT_ICON)).color(palette().accent);
            }
            if self.locked {
                icon(MaterialSymbol::LOCK).extent(Length::px(FOOT_ICON)).color(palette().text_muted);
            }
            text(self.name.clone())
                .subheading()
                .semibold()
                .no_wrap()
                .grow(1.0)
                .min_width(0.0);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
    }

    fn preview(&self) -> Element {
        let Some(author) = &self.author else {
            return text(tr!("discord.forum.starter-gone"))
                .body()
                .muted()
                .italic()
                .no_wrap()
                .fill_width()
                .any();
        };
        let excerpt = self.excerpt.clone().unwrap_or_default();
        row! {
            text(format!("{}:", author.name))
                .body()
                .semibold()
                .color(author.color.unwrap_or(palette().text))
                .no_wrap()
                .min_width(0.0);
            text(excerpt).body().muted().no_wrap().grow(1.0).min_width(0.0);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any()
    }

    fn foot(&self) -> Element {
        let badge = self.badge.as_ref().map(|badge| {
            let (thread, emoji, on) = (self.thread, badge.emoji.clone(), !badge.me);
            reaction_chip(
                badge,
                self.reacting.then(|| {
                    Handler::new(move |chat: &mut Chat| {
                        chat.react_to_post(thread, emoji.clone(), on)
                    })
                }),
            )
        });
        row! {
            if let Some(badge) = badge {
                badge;
            }
            icon(MaterialSymbol::CHAT_BUBBLE).extent(Length::px(FOOT_ICON)).color(palette().text_muted);
            text(self.replies.to_string()).caption().muted().no_wrap();
            if let Some(at) = self.at {
                text("•").caption().faint().no_wrap();
                component_with(|chat: &Chat| chat, at, since);
            }
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .any()
    }
}

fn since(chat: &Chat, at: &i64) -> Element {
    chat.conversation().clock();
    let now = jiff::Timestamp::now().as_second();
    text(time::relative(at / 1000, now))
        .caption()
        .muted()
        .no_wrap()
        .any()
}

fn excerpt_of(starter: &Message) -> String {
    starter.content.one_line().chars().take(EXCERPT).collect()
}

fn cover_of(starter: &Message) -> Option<Cover> {
    let attached = starter.attachments.iter().find_map(|attachment| {
        let picture = match attachment.kind {
            AttachmentKind::Image { .. } => Some(
                attachment
                    .preview
                    .clone()
                    .unwrap_or_else(|| Picture::fixed(attachment.url.clone())),
            ),
            AttachmentKind::Video { .. } => attachment.preview.clone(),
            AttachmentKind::Audio { .. } | AttachmentKind::File { .. } => None,
        }?;
        Some(match attachment.spoiler {
            true => Cover::Spoiler,
            false => Cover::Picture(picture),
        })
    });
    attached.or_else(|| {
        starter.embeds.iter().find_map(|embed| {
            embed
                .image
                .as_ref()
                .or(embed.thumbnail.as_ref())
                .map(|media| Cover::Picture(media.picture.clone()))
        })
    })
}

fn cover_view(cover: &Cover, width: Length, height: Length) -> Element {
    let side = width
        .fixed()
        .unwrap_or(COVER * 2.0)
        .max(height.fixed().unwrap_or(COVER));
    let picture = match cover {
        Cover::Picture(picture) => picture,
        Cover::Spoiler => return placeholder(MaterialSymbol::VISIBILITY_OFF, width, height),
    };
    match pictures::fetching(picture, side) {
        Fetch::Ready(bytes) => image((*bytes).clone())
            .fit(ImageFit::Cover)
            .size(width, height)
            .any(),
        Fetch::Failed(_) => placeholder(MaterialSymbol::BROKEN_IMAGE, width, height),
        Fetch::Loading => container(spinner())
            .center()
            .background(palette().surface_variant)
            .size(width, height)
            .any(),
    }
}

fn placeholder(symbol: MaterialSymbol, width: Length, height: Length) -> Element {
    container(
        icon(symbol)
            .extent(Length::px(24.0))
            .color(palette().text_muted),
    )
    .center()
    .background(palette().surface_variant)
    .size(width, height)
    .any()
}

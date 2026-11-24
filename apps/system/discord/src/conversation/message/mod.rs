mod attachments;
mod embeds;
mod extras;
mod hover;
mod reactions;
pub(crate) mod rich;

use std::rc::Rc;
use std::sync::Arc;

use discord_core::{Abilities, ChannelId, Delivery, Message, MessageId, SpaceId};
use rikka_app::prelude::*;
use rikka_ui::{Element, Handler};
use rikka_widgets::{column, row};

use crate::chat::Chat;
use crate::conversation::composer::sendable;
use crate::conversation::lightbox::{Album, Sender};
use crate::conversation::overlay::{Overlay, row_key};
use crate::conversation::time;
use crate::conversation::widgets::context_row;
use crate::widgets::avatar;
use extras::GUTTER;
use hover::Allowed;
use rich::{Reveal, Shaping};

pub(crate) use reactions::reaction_chip;

const FACE: f32 = 40.0;
const EDIT_SHORTEST: f32 = 22.0;
const EDIT_TALLEST: f32 = 320.0;

#[derive(Clone, Debug)]
pub(crate) struct Props {
    pub(crate) key: String,
    pub(crate) channel: ChannelId,
    pub(crate) message: Arc<Message>,
    pub(crate) head: bool,
    pub(crate) editing: bool,
    pub(crate) flashed: bool,
}

impl PartialEq for Props {
    fn eq(&self, other: &Props) -> bool {
        self.head == other.head
            && self.editing == other.editing
            && self.flashed == other.flashed
            && Arc::ptr_eq(&self.message, &other.message)
            && self.key == other.key
            && self.channel == other.channel
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Local {
    hovered: bool,
    revealed: u64,
}

pub(crate) fn view(chat: &Chat, props: &Props) -> Element {
    let Some(linked) = chat.linked(&props.key) else {
        return spacer().height(1.0).any();
    };
    let Some(channel_info) = linked.channel(&props.channel) else {
        return spacer().height(1.0).any();
    };
    let message = props.message.clone();
    let space: Option<SpaceId> = channel_info.space;
    if let Some(line) = extras::system_line(linked, space.as_ref(), &message) {
        return line;
    }
    let conversation = chat.conversation();
    let abilities = channel_info.abilities;
    let mine = linked.is_me(&message.author.id);
    let delivered = message.delivery == Delivery::Delivered;
    let (editing, flashed) = (props.editing, props.flashed);
    let name = linked.display_name(space.as_ref(), &message.author);
    let color = linked
        .color_of(space.as_ref(), &message.author.id)
        .map_or(palette().text, Color::hex);
    let appearance = chat.appearance();
    let compact = appearance.compact;
    let mut shaping = Shaping::new(linked, space.as_ref(), appearance.animated_emoji);
    let content = shaping.rich(&message.content);
    let embeds: Vec<embeds::Prepared> = match appearance.embeds {
        true => message
            .embeds
            .iter()
            .map(|embed| embeds::prepare(&mut shaping, embed))
            .collect(),
        false => Vec::new(),
    };
    if shaping.is_relative() {
        conversation.clock();
    }
    let reply = message
        .reply
        .as_ref()
        .map(|reply| extras::reply_line(linked, space.as_ref(), reply));
    let album = Album::new(
        Sender {
            name: name.clone(),
            avatar: message.author.avatar.clone(),
            at: message.at,
        },
        attachments::shots(conversation.media(), &message.attachments)
            .into_iter()
            .chain(embeds.iter().flat_map(embeds::shots))
            .collect(),
    );
    let attached = attachments::view(conversation, &message.attachments, &album);
    let stickers = extras::stickers(&message.stickers);
    let thread = extras::thread_chip(linked, &message);
    let reactions = reactions::view(
        &message.id,
        &message.reactions,
        delivered && abilities.contains(Abilities::REACT),
    );
    let sending = match message.delivery {
        Delivery::Sending => message
            .nonce
            .and_then(|nonce| conversation.outbox.progress(&nonce)),
        _ => None,
    };
    let allowed = Allowed {
        react: delivered && abilities.contains(Abilities::REACT),
        reply: delivered && sendable(channel_info),
        edit: delivered && mine,
    };
    let mentioned = message.mentions_me;
    let head = props.head;
    let editor = editing.then(|| {
        let draft = conversation.composing().edit_draft().clone();
        column! {
            text_editor(&draft)
                .gutter(false)
                .font_size(15.0)
                .line_height(22.0)
                .fits_text(EDIT_SHORTEST..=EDIT_TALLEST)
                .autofocus(true)
                .on_submit(|chat: &mut Chat| chat.save_edit())
                .on_key(|chat: &mut Chat, key: Key, modifiers: Modifiers| chat.edit_key(key, modifiers))
                .fill_width();
            text(tr!("discord.conversation.edit-tip")).caption().muted();
        }
        .gap(Space::HAIR)
        .padding(Inset::all(Space::TIGHT))
        .background(palette().surface)
        .fill_width()
        .any()
    });
    let author = message.author.clone();
    let failed = (message.delivery == Delivery::Failed)
        .then_some(message.nonce)
        .flatten();
    let at = message.at;
    let edited = message.edited.is_some();
    let id = message.id;
    let profile_space = space;
    local_state(Local::default(), move |local: Local, set: Setter<Local>| {
        let hovered = local.hovered;
        let reveal = Reveal {
            revealed: local.revealed,
            set: {
                let set = set.clone();
                Rc::new(move |revealed: u64| {
                    set(Local {
                        hovered: local.hovered,
                        revealed,
                    })
                })
            },
            subject: id.get(),
            space: profile_space,
        };
        let gutter = match (head && !compact, hovered || (head && compact)) {
            (true, _) => {
                let user = author.id;
                let space = profile_space;
                let anchor = ("discord-author", id);
                container(avatar(author.avatar.as_ref(), &name, FACE))
                    .focusless()
                    .on_press(move |chat: &mut Chat| {
                        chat.open_profile(&user, space.as_ref(), Anchor::beside(anchor))
                    })
                    .build()
                    .keyed(("discord-author", id))
            }
            (false, true) => container(text(time::short_clock(at)).caption().muted().no_wrap())
                .align_x(Align::Center)
                .padding(Inset::edges(
                    Space::TIGHT,
                    Space::NONE,
                    Space::NONE,
                    Space::NONE,
                ))
                .fill_width()
                .any(),
            (false, false) => spacer().any(),
        };
        let mut body: Vec<Element> = Vec::new();
        if head {
            let user = author.id;
            let space = profile_space;
            let anchor = ("discord-name", id);
            body.push(
                row! {
                    container(text(name.clone()).label().semibold().color(color).no_wrap())
                        .focusless()
                        .on_press(move |chat: &mut Chat| {
                            chat.open_profile(&user, space.as_ref(), Anchor::beside(anchor))
                        })
                        .build()
                        .keyed(("discord-name", id))
                        .min_width(0.0);
                    if author.bot {
                        container(text(tr!("discord.conversation.bot")).caption().bold().color(palette().on_accent))
                            .padding(Inset::xy(Space::TIGHT, Space::NONE))
                            .background(palette().accent)
                            .radius(Corner::Detail);
                    }
                    if !compact {
                        text(time::sent_at(at)).caption().muted().no_wrap();
                    }
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .any(),
            );
        }
        match editor {
            Some(editor) => body.push(editor),
            None if !content.is_empty() => {
                let rendered = rich::render(&content, &reveal);
                body.push(match edited {
                    true => column! {
                        rendered;
                        text(tr!("discord.conversation.edited")).caption().muted();
                    }
                    .fill_width()
                    .any(),
                    false => rendered,
                });
            }
            None => {}
        }
        body.extend(attached);
        if let Some(stickers) = stickers {
            body.push(stickers);
        }
        for embed in &embeds {
            body.push(embeds::render_embed(embed, &reveal, &album));
        }
        if let Some(reactions) = reactions {
            body.push(reactions);
        }
        if let Some(thread) = thread {
            body.push(thread);
        }
        if let Some(sending) = sending {
            body.push(sending);
        }
        if let Some(nonce) = failed {
            body.push(failed_row(nonce));
        }
        let main = column_of(body).gap(Space::TIGHT).grow(1.0).min_width(0.0);
        let mut layers: Vec<Element> = Vec::new();
        if let Some(reply) = reply {
            layers.push(reply);
        }
        layers.push(
            row! {
                container(gutter).width(GUTTER).align_x(Align::Start);
                main;
            }
            .align_items(Align::Start)
            .fill_width()
            .any(),
        );
        if hovered && !editing {
            layers.push(
                row! {
                    spacer().grow(1.0);
                    hover::bar(&id, allowed);
                }
                .padding(Inset::edges(
                    Space::NONE,
                    Space::ROOMY,
                    Space::NONE,
                    Space::NONE,
                ))
                .fill_width()
                .position(0.0, -Space::NORMAL.px())
                .any(),
            );
        }
        let tint = match () {
            _ if flashed => palette().accent.with_alpha(0.14),
            _ if mentioned => palette().warning.with_alpha(0.10),
            _ if hovered => palette().text.with_alpha(0.035),
            _ => Color::TRANSPARENT,
        };
        let opacity = match delivered || failed.is_some() {
            true => 1.0,
            false => 0.55,
        };
        let open_menu = {
            let id = id;
            Handler::new(move |overlay: &mut Overlay, modifiers: Modifiers| {
                overlay.set_menu_target(id, modifiers.shift)
            })
        };
        let row = container(column_of(layers).fill_width())
            .padding(Inset::edges(
                match (head, compact) {
                    (true, false) => Space::ROOMY,
                    (true, true) => Space::TIGHT,
                    (false, _) => Space::HAIR,
                },
                Space::ROOMY,
                Space::HAIR,
                Space::ROOMY,
            ))
            .background(tint)
            .on_hover(move |inside: bool| {
                set(Local {
                    hovered: inside,
                    revealed: local.revealed,
                });
                skip_view();
            })
            .fill_width()
            .opacity(opacity)
            .build()
            .keyed(row_key(&id));
        context_row(open_menu, row).fill_width()
    })
}

fn failed_row(nonce: MessageId) -> Element {
    row! {
        icon(MaterialSymbol::ERROR).extent(Length::px(16.0)).color(palette().danger);
        text(tr!("discord.conversation.failed")).caption().danger();
        button(text(tr!("discord.conversation.retry")).caption())
            .small()
            .ghost()
            .on_press(move |chat: &mut Chat| chat.retry_message(&nonce));
        button(text(tr!("discord.conversation.discard")).caption())
            .small()
            .ghost()
            .on_press(move |chat: &mut Chat| chat.discard_message(&nonce));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .any()
}

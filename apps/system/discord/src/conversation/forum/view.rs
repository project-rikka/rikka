use std::ops::Range;
use std::rc::Rc;

use discord_core::{Abilities, Channel, ChannelId, ForumSettings, PostLayout};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use super::card::Card;
use super::{Forum, Posts, SORT_BUTTON, compose, tags};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::Conversation;
use crate::conversation::overlay::{Overlay, Shown};

const TILE: f32 = 248.0;
const TAIL: u64 = u64::MAX;
const SEARCH_ICON: f32 = 22.0;
const SEARCH_SIZE: f32 = 16.0;
const SEARCH_CHARS: usize = 100;

pub(crate) fn view(chat: &Chat, forum: &ChannelId) -> Element {
    column! {
        if let Some(error_message) = chat.conversation().error_message() {
            banner(error_message.to_owned())
                .on_dismiss(tr!("discord.conversation.dismiss"), |conversation: &mut Conversation| {
                    conversation.dismiss()
                })
                .fill_width();
        }
        column! {
            component_with(|chat: &Chat| chat, *forum, head);
            component_with(|chat: &Chat| chat, *forum, controls);
        }
        .gap(Space::NORMAL)
        .padding(Inset::edges(Space::ROOMY, Space::ROOMY, Space::SNUG, Space::ROOMY))
        .fill_width();
        component_with(|chat: &Chat| chat, *forum, listing)
            .grow(1.0)
            .fill_width();
    }
    .fill()
    .any()
}

fn shown<'a>(
    chat: &'a Chat,
    forum: &ChannelId,
) -> Option<(&'a LinkedAccount, &'a Channel, &'a ForumSettings)> {
    let linked = chat.current()?;
    let forum_info = linked.channel(forum)?;
    Some((linked, forum_info, forum_info.forum.as_ref()?))
}

fn head(chat: &Chat, forum: &ChannelId) -> Element {
    let Some((_, forum_info, settings)) = shown(chat, forum) else {
        return spacer().any();
    };
    let state = chat.conversation().forum();
    match *state.writing {
        true => compose::view(forum_info, settings, state),
        false => search_bar(state, forum_info.abilities.contains(Abilities::SEND)),
    }
}

fn listing(chat: &Chat, forum: &ChannelId) -> Element {
    let Some((linked, forum_info, settings)) = shown(chat, forum) else {
        return spacer().fill().any();
    };
    let state = chat.conversation().forum();
    let reacting = forum_info.abilities.contains(Abilities::REACT);
    let sort = state.view.sort_in(settings);
    let mut cards: Vec<Card> = state
        .posts
        .list
        .iter()
        .map(|post| Card::new(linked, settings, post, sort, reacting))
        .collect();
    cards.sort_by_key(|card| !card.is_pinned());
    let narrowed = !state.searched.is_empty() || !state.filter.is_empty();
    posts_view(
        &state.posts,
        cards,
        state.view.layout_in(settings),
        narrowed,
    )
}

fn search_bar(state: &Forum, can_post: bool) -> Element {
    container(
        row! {
            icon(MaterialSymbol::SEARCH).extent(Length::px(SEARCH_ICON)).color(palette().text_muted);
            text_field(&state.search)
                .placeholder(tr!("discord.forum.search"))
                .plain(true)
                .font_size(SEARCH_SIZE)
                .max_len(SEARCH_CHARS)
                .on_change(|chat: &mut Chat| chat.search_typed())
                .on_submit(|chat: &mut Chat| chat.search_now())
                .grow(1.0)
                .min_width(0.0);
            if *state.typed {
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(18.0)))
                    .icon_only()
                    .ghost()
                    .small()
                    .focusless()
                    .on_press(|chat: &mut Chat| chat.clear_search());
            }
            if can_post {
                button(
                    row! {
                        icon(MaterialSymbol::CHAT_BUBBLE).extent(Length::px(18.0));
                        text(tr!("discord.forum.new-post")).no_wrap();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .primary()
                .on_press(|chat: &mut Chat| chat.start_post());
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .background(palette().surface)
    .border(1.0, palette().outline)
    .radius(Corner::Panel)
    .fill_width()
    .any()
}

fn controls(chat: &Chat, forum: &ChannelId) -> Element {
    let Some((_, _, settings)) = shown(chat, forum) else {
        return spacer().any();
    };
    let conversation = chat.conversation();
    let sorting = matches!(conversation.overlay().shown(), Some(Shown::SortView));
    let filter = &conversation.forum().filter;
    let sort_button = tags::pill(
        row! {
            icon(MaterialSymbol::SWAP_VERT).extent(Length::px(18.0)).color(palette().text_muted);
            text(tr!("discord.forum.sort-view")).caption().semibold().no_wrap();
            icon(MaterialSymbol::EXPAND_MORE).extent(Length::px(18.0)).color(palette().text_muted);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
        sorting,
    )
    .on_press(|overlay: &mut Overlay| overlay.toggle(Shown::SortView))
    .build()
    .keyed(SORT_BUTTON);
    row! {
        sort_button;
        if !settings.tags.is_empty() {
            rule_upright().height(Length::px(20.0));
            tags::filter_bar(&settings.tags, filter).grow(1.0).min_width(0.0);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn posts_view(posts: &Posts, cards: Vec<Card>, layout: PostLayout, narrowed: bool) -> Element {
    match (&posts.error_message, cards.is_empty(), posts.loading) {
        (Some(error_message), true, _) => {
            return container(
                column! {
                    status_page(MaterialSymbol::ERROR, error_message.clone())
                        .tint(palette().danger);
                    button(text(tr!("discord.forum.retry")))
                        .secondary()
                        .on_press(|chat: &mut Chat| chat.retry_posts());
                }
                .gap(Space::NORMAL)
                .align_items(Align::Center),
            )
            .center()
            .grow(1.0)
            .fill_width()
            .any();
        }
        (None, true, true) => {
            return container(spinner()).center().grow(1.0).fill_width().any();
        }
        (None, true, false) => {
            let (symbol, caption) = match narrowed {
                true => (MaterialSymbol::SEARCH_OFF, tr!("discord.forum.no-results")),
                false => (MaterialSymbol::FORUM, tr!("discord.forum.no-posts")),
            };
            return status_page(symbol, caption).grow(1.0).any();
        }
        _ => {}
    }
    let tail = (posts.more || posts.error_message.is_some()).then(|| posts.error_message.clone());
    let count = cards.len() + usize::from(tail.is_some());
    let names: Vec<u64> = cards.iter().map(Card::name).collect();
    let cards = Rc::new(cards);
    let cell = move |index: usize| match cards.get(index) {
        Some(card) => match layout {
            PostLayout::List => card.row(),
            PostLayout::Gallery => card.tile(),
        },
        None => tail_view(tail.clone().flatten()),
    };
    let listed = match layout {
        PostLayout::List => lazy_column(count, move |slot| cell(slot.index))
            .named(move |index| names.get(index).copied().unwrap_or(TAIL))
            .gap(Space::SNUG),
        PostLayout::Gallery => {
            lazy_tiles(Length::px(TILE), count, move |slot| cell(slot.index)).gap(Space::NORMAL)
        }
    };
    listed
        .padding(Inset::edges(
            Space::SNUG,
            Space::ROOMY,
            Space::ROOMY,
            Space::ROOMY,
        ))
        .on_visible(|chat: &mut Chat, shown: Range<usize>| chat.posts_shown(shown))
        .grow(1.0)
        .fill_width()
        .any()
}

fn tail_view(error_message: Option<String>) -> Element {
    let body = match error_message {
        Some(error_message) => row! {
            text(error_message).caption().danger().no_wrap().min_width(0.0);
            button(text(tr!("discord.forum.retry")))
                .secondary()
                .small()
                .on_press(|chat: &mut Chat| chat.retry_posts());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .any(),
        None => spinner().any(),
    };
    container(body)
        .center()
        .padding(Inset::all(Space::NORMAL))
        .fill_width()
        .any()
}

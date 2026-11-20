use std::cmp::Reverse;
use std::rc::Rc;

use discord_core::{
    Abilities, Channel, ChannelId, Draft, ForumPage, ForumPost, ForumTag, Limits, TagId, Upload,
    unwatched,
};
use rikka_app::prelude::*;
use rikka_ui::{DragAction, DragOffer, Element};
use rikka_widgets::{column, row};

use crate::chat::Chat;
use crate::conversation::composer::preview;
use crate::conversation::composer::uploads::{self, Attach, Pending};
use crate::conversation::{Seat, time};

const CARD_EXCERPT: usize = 220;
const PREVIEW: f32 = 72.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sort {
    Activity,
    Created,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Posts {
    pub(crate) list: Vec<ForumPost>,
    pub(crate) more: bool,
    pub(crate) loading: bool,
    pub(crate) error_message: Option<String>,
}

#[derive(State)]
pub(crate) struct Forum {
    #[untracked]
    seat: Option<Seat>,
    #[untracked]
    title: TextDocument,
    #[untracked]
    body: TextDocument,
    #[untracked]
    next_upload: u64,
    posts: Tracked<Posts>,
    sort: Tracked<Sort>,
    filter: Tracked<Option<TagId>>,
    writing: Tracked<bool>,
    chosen: Tracked<Vec<TagId>>,
    uploads: Tracked<Vec<Pending>>,
    posting: Tracked<bool>,
    refused: Tracked<Option<String>>,
}

impl Forum {
    pub(crate) fn new() -> Forum {
        Forum {
            seat: None,
            title: TextDocument::new(""),
            body: TextDocument::new(""),
            next_upload: 0,
            posts: Tracked::new(Posts::default()),
            sort: Tracked::new(Sort::Activity),
            filter: Tracked::new(None),
            writing: Tracked::new(false),
            chosen: Tracked::new(Vec::new()),
            uploads: Tracked::new(Vec::new()),
            posting: Tracked::new(false),
            refused: Tracked::new(None),
        }
    }

    pub(crate) fn posts(&self) -> &Posts {
        &self.posts
    }

    pub(crate) fn is_writing(&self) -> bool {
        *self.writing
    }

    pub(crate) fn refused(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    pub(crate) fn title(&self) -> &TextDocument {
        &self.title
    }

    pub(crate) fn body(&self) -> &TextDocument {
        &self.body
    }

    pub(crate) fn write(&mut self) {
        self.writing.set(true);
    }

    pub(crate) fn cancel(&mut self) {
        self.writing.set(false);
        self.title.clear();
        self.body.clear();
        self.chosen.set(Vec::new());
        self.uploads.set(Vec::new());
        self.refused.set(None);
        self.posting.set(false);
    }

    pub(crate) fn sort_by(&mut self, sort: Sort) {
        self.sort.set(sort);
    }

    pub(crate) fn filter_by(&mut self, tag: Option<TagId>) {
        let tag = match *self.filter == tag {
            true => None,
            false => tag,
        };
        self.filter.set(tag);
    }

    pub(crate) fn toggle_tag(&mut self, tag: &TagId) {
        match self.chosen.iter().position(|chosen| chosen == tag) {
            Some(at) => {
                self.chosen.remove(at);
            }
            None => self.chosen.push(*tag),
        }
    }

    pub(crate) fn remove_upload(&mut self, id: u64) {
        self.uploads.retain(|pending| pending.id != id);
    }

    pub(crate) fn attached(
        &mut self,
        picked: Vec<Upload>,
        mut refused: Vec<String>,
        limits: Limits,
    ) {
        let (admitted, more) = uploads::admit(&self.uploads, picked, limits, &mut self.next_upload);
        refused.extend(more);
        if !admitted.is_empty() {
            self.uploads.extend(admitted);
        }
        self.refused
            .set((!refused.is_empty()).then(|| refused.join("\n")));
    }

    fn enter(&mut self, seat: &Seat) -> bool {
        if self.seat.as_ref() == Some(seat) {
            return false;
        }
        self.seat = Some(seat.clone());
        self.cancel();
        self.filter.set(None);
        self.posts.set(Posts::default());
        true
    }
}

impl std::fmt::Debug for Forum {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Forum")
            .field("seat", &self.seat)
            .field("writing", &*self.writing)
            .finish_non_exhaustive()
    }
}

impl Chat {
    pub(crate) fn open_forum(&mut self, key: &str, forum: &ChannelId) {
        let seat = (key.to_owned(), *forum);
        if self.conversation_mut().forum.enter(&seat) {
            self.load_posts(key, forum, 0);
        }
    }

    pub(crate) fn more_posts(&mut self) {
        let Some((key, forum)) = self.conversation().forum.seat.clone() else {
            return;
        };
        let offset = u32::try_from(self.conversation().forum.posts.list.len()).unwrap_or(u32::MAX);
        self.load_posts(&key, &forum, offset);
    }

    fn load_posts(&mut self, key: &str, forum: &ChannelId, offset: u32) {
        let Some(client) = self.linked(key).map(|linked| linked.client().clone()) else {
            return;
        };
        let posts = &mut self.conversation_mut().forum.posts;
        if posts.loading {
            return;
        }
        posts.loading = true;
        let (landed_key, landed_forum) = (key.to_owned(), *forum);
        let forum_id = *forum;
        spawn_keyed(
            ("discord-forum", key.to_owned(), *forum),
            async move { client.forum_posts(&forum_id, offset).await },
            move |chat: &mut Chat, page: discord_core::Result<ForumPage>| {
                chat.on_posts_loaded(&landed_key, &landed_forum, offset, page);
            },
        );
    }

    pub(crate) fn on_posts_loaded(
        &mut self,
        key: &str,
        forum: &ChannelId,
        offset: u32,
        page: discord_core::Result<ForumPage>,
    ) {
        let seat = (key.to_owned(), *forum);
        let forum_state = &mut self.conversation_mut().forum;
        if forum_state.seat.as_ref() != Some(&seat) {
            return;
        }
        let posts = &mut *forum_state.posts;
        posts.loading = false;
        match page {
            Ok(page) => {
                if offset == 0 {
                    posts.list.clear();
                }
                for post in page.posts {
                    if !posts
                        .list
                        .iter()
                        .any(|known| known.thread.id == post.thread.id)
                    {
                        posts.list.push(post);
                    }
                }
                posts.more = page.more;
                posts.error_message = None;
            }
            Err(error) => posts.error_message = Some(error.to_string()),
        }
    }

    pub(crate) fn submit_post(&mut self) {
        let Some((key, forum)) = self.conversation().forum.seat.clone() else {
            return;
        };
        let state = &self.conversation().forum;
        if *state.posting {
            return;
        }
        let title = state.title.text().trim().to_owned();
        let text = state.body.text().trim().to_owned();
        if title.is_empty() {
            let caption = tr!("discord.conversation.post-needs-title").to_string();
            self.conversation_mut().forum.refused.set(Some(caption));
            return;
        }
        if text.is_empty() && state.uploads.is_empty() {
            let caption = tr!("discord.conversation.post-needs-body").to_string();
            self.conversation_mut().forum.refused.set(Some(caption));
            return;
        }
        let tags = state.chosen.to_vec();
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let state = &mut self.conversation_mut().forum;
        let uploads: Vec<Upload> = state
            .uploads
            .iter()
            .map(|pending| pending.upload.clone())
            .collect();
        state.posting.set(true);
        state.refused.set(None);
        let draft = Draft {
            nonce: client.mint(),
            text,
            reply: None,
            uploads,
            stickers: Vec::new(),
            silent: false,
        };
        let (landed_key, landed_forum) = (key.clone(), forum);
        spawn(
            async move {
                client
                    .create_post(&forum, &title, &tags, draft, unwatched())
                    .await
            },
            move |chat: &mut Chat, posted: discord_core::Result<Channel>| {
                chat.posted(&landed_key, &landed_forum, posted);
            },
        );
    }

    pub(crate) fn posted(
        &mut self,
        key: &str,
        forum: &ChannelId,
        posted: discord_core::Result<Channel>,
    ) {
        let seat = (key.to_owned(), *forum);
        if self.conversation().forum.seat.as_ref() != Some(&seat) {
            return;
        }
        match posted {
            Ok(thread) => {
                self.conversation_mut().forum.cancel();
                self.conversation_mut().forum.seat = None;
                self.open_thread(key, thread);
            }
            Err(error) => {
                let state = &mut self.conversation_mut().forum;
                state.posting.set(false);
                state.refused.set(Some(error.to_string()));
            }
        }
    }
}

pub(crate) fn view(chat: &Chat, forum: &ChannelId) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().fill().any();
    };
    let Some(forum_info) = linked.channel(forum) else {
        return spacer().fill().any();
    };
    let state = chat.conversation().forum();
    let posts = state.posts();
    let sort = *state.sort;
    let filter = state.filter.clone();
    let mut shown: Vec<ForumPost> = posts
        .list
        .iter()
        .filter(|post| {
            filter.as_ref().is_none_or(|tag| {
                post.thread
                    .thread
                    .as_ref()
                    .is_some_and(|thread| thread.applied_tags.contains(tag))
            })
        })
        .cloned()
        .collect();
    match sort {
        Sort::Activity => shown.sort_by_key(|post| Reverse(post.thread.last_activity)),
        Sort::Created => shown.sort_by(|left, right| {
            let created = |post: &ForumPost| {
                post.thread
                    .thread
                    .as_ref()
                    .and_then(|thread| thread.created)
            };
            created(right).cmp(&created(left))
        }),
    }
    let tags = Rc::new(forum_info.tags.clone());
    let key = linked.key().to_owned();
    let space = forum_info.space;
    let names: Vec<String> = shown
        .iter()
        .map(|post| {
            post.starter
                .as_ref()
                .map(|starter| linked.display_name(space.as_ref(), &starter.author))
                .unwrap_or_default()
        })
        .collect();
    let more = posts.more;
    let count = shown.len() + usize::from(more);
    let loading = posts.loading;
    let shown = Rc::new(shown);
    let names = Rc::new(names);
    let listed = {
        let tags = tags.clone();
        lazy_column(count, move |slot| match shown.get(slot.index) {
            Some(post) => card(&key, post, &names[slot.index], &tags),
            None => container(
                button(text(tr!("discord.conversation.more-posts")))
                    .secondary()
                    .loading(loading)
                    .on_press(|chat: &mut Chat| chat.more_posts()),
            )
            .center()
            .padding(Inset::all(Space::NORMAL))
            .fill_width()
            .any(),
        })
        .gap(Space::SNUG)
        .padding(Inset::all(Space::ROOMY))
        .grow(1.0)
        .fill_width()
        .any()
    };
    let body = match (posts.list.is_empty(), loading, &posts.error_message) {
        (_, _, Some(error_message)) => status_page(MaterialSymbol::ERROR, error_message.clone())
            .tint(palette().danger)
            .grow(1.0)
            .any(),
        (true, true, None) => container(spinner()).center().grow(1.0).fill_width().any(),
        (true, false, None) => {
            status_page(MaterialSymbol::FORUM, tr!("discord.conversation.no-posts"))
                .grow(1.0)
                .any()
        }
        _ => listed,
    };
    let can_post = forum_info.abilities.contains(Abilities::SEND)
        || forum_info.abilities.contains(Abilities::SEND_IN_THREADS);
    column! {
        row! {
            if can_post && !state.is_writing() {
                button(
                    row! {
                        icon(MaterialSymbol::ADD).extent(Length::px(18.0));
                        text(tr!("discord.conversation.new-post"));
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .primary()
                .on_press(|forum: &mut Forum| forum.write());
            }
            spacer().grow(1.0);
            segmented(match sort {
                Sort::Activity => 0,
                Sort::Created => 1,
            })
            .segment(tr!("discord.conversation.sort-activity"))
            .segment(tr!("discord.conversation.sort-created"))
            .on_select(|forum: &mut Forum, picked: usize| {
                forum.sort_by(match picked {
                    0 => Sort::Activity,
                    _ => Sort::Created,
                })
            })
            .width(280.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::ROOMY, Space::SNUG))
        .fill_width();
        if !forum_info.tags.is_empty() {
            tag_filter(&forum_info.tags, state.filter.as_ref());
        }
        if state.is_writing() {
            form(state, forum_info);
        }
        body;
    }
    .fill()
    .any()
}

fn tag_chip(tag: &ForumTag, lit: bool) -> Container {
    container(
        row! {
            if let Some(emoji) = &tag.emoji {
                crate::conversation::emoji::face(emoji, 14.0);
            }
            text(tag.name.clone()).caption().semibold().no_wrap();
        }
        .gap(Space::HAIR)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::SNUG, Space::HAIR))
    .background(match lit {
        true => palette().accent.with_alpha(0.2),
        false => palette().surface_variant,
    })
    .border(
        1.0,
        match lit {
            true => palette().accent,
            false => Color::TRANSPARENT,
        },
    )
    .radius(Corner::Control)
}

fn tag_filter(tags: &[ForumTag], filter: Option<&TagId>) -> Element {
    row_of(tags.iter().map(|tag| {
        let id = tag.id;
        tag_chip(tag, filter == Some(&tag.id))
            .focusless()
            .on_press(move |forum: &mut Forum| forum.filter_by(Some(id)))
            .any()
    }))
    .gap(Space::TIGHT)
    .wrap_items()
    .padding(Inset::xy(Space::ROOMY, Space::NONE))
    .fill_width()
    .any()
}

fn card(key: &str, post: &ForumPost, author: &str, tags: &[ForumTag]) -> Element {
    let applied: Vec<&ForumTag> = post
        .thread
        .thread
        .as_ref()
        .map(|thread| {
            tags.iter()
                .filter(|tag| thread.applied_tags.contains(&tag.id))
                .collect()
        })
        .unwrap_or_default();
    let excerpt: String = post
        .starter
        .as_ref()
        .map(|starter| starter.content.plain_text())
        .unwrap_or_default()
        .chars()
        .take(CARD_EXCERPT)
        .collect();
    let active = post
        .thread
        .last_activity
        .map(time::sent_at)
        .unwrap_or_default();
    let (key, thread) = (key.to_owned(), post.thread.clone());
    container(
        column! {
            text(post.thread.name.clone()).label().semibold().fill_width();
            if !applied.is_empty() {
                row_of(applied.iter().map(|tag| tag_chip(tag, false).any()))
                    .gap(Space::TIGHT)
                    .wrap_items();
            }
            if !excerpt.is_empty() {
                text(excerpt).body().muted().fill_width();
            }
            row! {
                if !author.is_empty() {
                    text(author.to_owned()).caption().semibold().no_wrap();
                }
                icon(MaterialSymbol::CHAT_BUBBLE).extent(Length::px(14.0)).color(palette().text_muted);
                text(post.replies.to_string()).caption().muted().no_wrap();
                text(tr!("discord.conversation.last-activity", at = active.as_str()))
                    .caption()
                    .muted()
                    .no_wrap()
                    .min_width(0.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::TIGHT)
        .fill_width(),
    )
    .padding(Inset::all(Space::NORMAL))
    .background(palette().surface)
    .radius(Corner::Item)
    .focusless()
    .on_press(move |chat: &mut Chat| chat.open_thread(&key, thread.clone()))
    .fill_width()
    .any()
}

fn form(state: &Forum, forum: &Channel) -> Element {
    let posting = *state.posting;
    let attaching = forum.abilities.contains(Abilities::ATTACH);
    let manages = forum.abilities.contains(Abilities::MANAGE_MESSAGES);
    let chosen = state.chosen.clone();
    let tags = row_of(
        forum
            .tags
            .iter()
            .filter(|tag| !tag.moderated || manages)
            .map(|tag| {
                let id = tag.id;
                tag_chip(tag, chosen.contains(&tag.id))
                    .focusless()
                    .on_press(move |forum: &mut Forum| forum.toggle_tag(&id))
                    .any()
            }),
    )
    .gap(Space::TIGHT)
    .wrap_items()
    .fill_width();
    let strip = row_of(state.uploads.iter().map(|pending| {
        let id = pending.id;
        column! {
            preview(pending, PREVIEW);
            row! {
                text(pending.upload.name.clone()).caption().no_wrap().grow(1.0).min_width(0.0);
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                    .icon_only()
                    .ghost()
                    .small()
                    .on_press(move |forum: &mut Forum| forum.remove_upload(id));
            }
            .align_items(Align::Center)
            .width(PREVIEW);
        }
        .gap(Space::HAIR)
        .any()
    }))
    .gap(Space::SNUG)
    .wrap_items()
    .fill_width();
    let body = column! {
        text(tr!("discord.conversation.new-post")).heading().semibold();
        text_field(state.title())
            .placeholder(tr!("discord.conversation.post-title"))
            .autofocus(true)
            .fill_width();
        if !forum.tags.is_empty() {
            tags;
        }
        text_editor(state.body())
            .placeholder(tr!("discord.conversation.post-body"))
            .gutter(false)
            .font_size(15.0)
            .line_height(22.0)
            .fits_text(66.0..=240.0)
            .on_paste(|chat: &mut Chat, offered: Vec<String>| chat.paste_into(Attach::Post, offered))
            .fill_width();
        if !state.uploads.is_empty() {
            strip;
        }
        if let Some(refused) = state.refused() {
            text(refused.to_owned()).caption().danger().fill_width();
        }
        row! {
            if attaching {
                button(
                    row! {
                        icon(MaterialSymbol::ATTACH_FILE).extent(Length::px(18.0));
                        text(tr!("discord.conversation.attach"));
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .ghost()
                .on_press(|chat: &mut Chat| chat.attach(Attach::Post));
            }
            spacer().grow(1.0);
            button(text(tr!("discord.conversation.cancel")))
                .ghost()
                .on_press(|forum: &mut Forum| forum.cancel());
            button(text(tr!("discord.conversation.post")))
                .primary()
                .loading(posting)
                .on_press(|chat: &mut Chat| chat.submit_post());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width();
    let card = container(
        drop_zone(body)
            .accepts(move |offer: &DragOffer| attaching.then(|| uploads::accepts(offer)).flatten())
            .on_drop(|chat: &mut Chat, offer: DragOffer, action: DragAction| {
                chat.drop_into(Attach::Post, offer, action)
            })
            .fill_width(),
    )
    .padding(Inset::all(Space::NORMAL))
    .background(palette().surface)
    .radius(Corner::Control)
    .fill_width();
    container(card)
        .padding(Inset::xy(Space::ROOMY, Space::SNUG))
        .fill_width()
        .any()
}

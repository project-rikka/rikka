mod card;
mod compose;
mod loading;
mod menus;
mod posting;
mod tags;
mod view;

use discord_core::{
    ChannelId, Emoji, ForumPost, ForumSettings, Limits, MessageId, PostQuery, ReactionKind, TagId,
    Upload,
};
use rikka_app::prelude::*;

use crate::conversation::Seat;
use crate::conversation::composer::Picked;
use crate::conversation::composer::uploads::{self, Pending};
use crate::preferences::ForumView;

pub(crate) use menus::sort_menu;
pub(crate) use view::view;

pub(crate) const EMOJI_BUTTON: &str = "discord-post-emoji";
pub(crate) const SORT_BUTTON: &str = "discord-forum-sort";

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Posts {
    query: PostQuery,
    list: Vec<ForumPost>,
    received: u32,
    more: bool,
    loading: bool,
    error_message: Option<String>,
}

#[derive(State)]
pub(crate) struct Forum {
    #[untracked]
    seat: Option<Seat>,
    #[untracked]
    search: TextDocument,
    #[untracked]
    title: TextDocument,
    #[untracked]
    body: TextDocument,
    #[untracked]
    picked: Vec<(String, Picked)>,
    #[untracked]
    next_upload: u64,
    posts: Tracked<Posts>,
    view: Tracked<ForumView>,
    filter: Tracked<Vec<TagId>>,
    searched: Tracked<String>,
    typed: Tracked<bool>,
    writing: Tracked<bool>,
    filled: Tracked<bool>,
    chosen: Tracked<Vec<TagId>>,
    uploads: Tracked<Vec<Pending>>,
    posting: Tracked<bool>,
    refused: Tracked<Option<String>>,
}

impl Forum {
    pub(crate) fn new() -> Forum {
        Forum {
            seat: None,
            search: TextDocument::new(""),
            title: TextDocument::new(""),
            body: TextDocument::new(""),
            picked: Vec::new(),
            next_upload: 0,
            posts: Tracked::new(Posts::default()),
            view: Tracked::new(ForumView::default()),
            filter: Tracked::new(Vec::new()),
            searched: Tracked::new(String::new()),
            typed: Tracked::new(false),
            writing: Tracked::new(false),
            filled: Tracked::new(false),
            chosen: Tracked::new(Vec::new()),
            uploads: Tracked::new(Vec::new()),
            posting: Tracked::new(false),
            refused: Tracked::new(None),
        }
    }

    fn cancel(&mut self) {
        self.writing.set(false);
        self.title.clear();
        self.body.clear();
        self.picked.clear();
        self.chosen.set(Vec::new());
        self.uploads.set(Vec::new());
        self.filled.set(false);
        self.refused.set(None);
        self.posting.set(false);
    }

    fn refill(&mut self) {
        let filled = !self.title.text().trim().is_empty()
            && (!self.body.text().trim().is_empty() || !self.uploads.is_empty());
        self.filled.set(filled);
    }

    fn toggle_tag(&mut self, tag: TagId) {
        match self.chosen.iter().position(|chosen| *chosen == tag) {
            Some(at) => {
                self.chosen.remove(at);
            }
            None if self.chosen.len() < ForumSettings::MAX_POST_TAGS => self.chosen.push(tag),
            None => skip_view(),
        }
    }

    fn remove_upload(&mut self, id: u64) {
        self.uploads.retain(|pending| pending.id != id);
        self.refill();
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
        self.refill();
    }

    fn ready(&self, settings: &ForumSettings) -> bool {
        *self.filled && (!settings.requires_tag || !self.chosen.is_empty())
    }

    fn enter(&mut self, seat: &Seat, view: ForumView) -> bool {
        if self.seat.as_ref() == Some(seat) {
            return false;
        }
        self.seat = Some(seat.clone());
        self.cancel();
        self.search.clear();
        self.typed.set(false);
        self.searched.set(String::new());
        self.filter.set(Vec::new());
        self.view.set(view);
        self.posts.set(Posts::default());
        true
    }

    fn react(&mut self, thread: &ChannelId, emoji: &Emoji, on: bool) -> Option<MessageId> {
        let at = self
            .posts
            .list
            .iter()
            .position(|post| post.thread.id == *thread)?;
        let starter = self.posts.list[at].starter.as_ref()?;
        let mine = starter
            .reactions
            .iter()
            .any(|reaction| reaction.me && reaction.emoji.same(emoji));
        if mine == on {
            return None;
        }
        let id = starter.id;
        let (emoji, me) = (emoji.clone(), true);
        let kind = match on {
            true => ReactionKind::Added { emoji, me },
            false => ReactionKind::Removed { emoji, me },
        };
        let starter = self.posts.list[at].starter.as_mut()?;
        kind.apply(&mut starter.reactions);
        Some(id)
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

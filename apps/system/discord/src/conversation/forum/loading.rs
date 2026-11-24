use std::ops::Range;
use std::time::Duration;

use discord_core::{ChannelId, ForumPage, ForumSettings, PostLayout, PostQuery, PostSort, TagId};
use rikka_app::prelude::*;

use super::Posts;
use crate::chat::Chat;
use crate::conversation::Seat;
use crate::preferences::ForumView;

const SEARCH_SETTLE: Duration = Duration::from_millis(300);
const SEARCH_TIMER: &str = "discord-forum-search";
const AHEAD: usize = 6;

impl Chat {
    pub(crate) fn open_forum(&mut self, key: &str, forum: &ChannelId) {
        let seat = (key.to_owned(), *forum);
        let view = self.forum_view(key, forum);
        if self.conversation_mut().forum.enter(&seat, view) {
            self.reload_posts();
        }
    }

    pub(super) fn shown_forum(&self) -> Option<(Seat, &ForumSettings)> {
        let seat = self.conversation().forum.seat.clone()?;
        let settings = self.linked(&seat.0)?.channel(&seat.1)?.forum.as_ref()?;
        Some((seat, settings))
    }

    fn reload_posts(&mut self) {
        let Some(((key, forum), sort, tag_match)) = self
            .shown_forum()
            .map(|(seat, settings)| (seat, settings.sort, settings.tag_match))
        else {
            return;
        };
        let state = &mut self.conversation_mut().forum;
        let query = PostQuery {
            sort: state.view.sort.unwrap_or(sort),
            tags: state.filter.to_vec(),
            tag_match,
            name: state.searched.to_string(),
        };
        state.posts.set(Posts {
            query: query.clone(),
            loading: true,
            ..Posts::default()
        });
        self.fetch_posts(&key, &forum, query, 0);
    }

    pub(crate) fn posts_shown(&mut self, shown: Range<usize>) {
        let posts = &self.conversation().forum.posts;
        if !posts.more
            || posts.loading
            || posts.error_message.is_some()
            || shown.end + AHEAD < posts.list.len()
        {
            return;
        }
        self.more_posts();
    }

    pub(crate) fn retry_posts(&mut self) {
        if self.conversation().forum.posts.list.is_empty() {
            self.reload_posts();
            return;
        }
        self.conversation_mut().forum.posts.error_message = None;
        self.more_posts();
    }

    fn more_posts(&mut self) {
        let Some((key, forum)) = self.conversation().forum.seat.clone() else {
            return;
        };
        let posts = &self.conversation().forum.posts;
        if posts.loading || !posts.more {
            return;
        }
        let (query, offset) = (posts.query.clone(), posts.received);
        self.conversation_mut().forum.posts.loading = true;
        self.fetch_posts(&key, &forum, query, offset);
    }

    fn fetch_posts(&mut self, key: &str, forum: &ChannelId, query: PostQuery, offset: u32) {
        let Some(client) = self.linked(key).map(|linked| linked.client().clone()) else {
            return;
        };
        let (landed_key, landed_forum, landed_query) = (key.to_owned(), *forum, query.clone());
        let forum_id = *forum;
        spawn_keyed(
            ("discord-forum", key.to_owned(), *forum),
            async move { client.forum_posts(&forum_id, &query, offset).await },
            move |chat: &mut Chat, page: discord_core::Result<ForumPage>| {
                chat.on_posts_loaded(&landed_key, &landed_forum, &landed_query, page);
            },
        );
    }

    fn on_posts_loaded(
        &mut self,
        key: &str,
        forum: &ChannelId,
        query: &PostQuery,
        page: discord_core::Result<ForumPage>,
    ) {
        let seat = (key.to_owned(), *forum);
        let state = &mut self.conversation_mut().forum;
        if state.seat.as_ref() != Some(&seat) || state.posts.query != *query {
            return;
        }
        let posts = &mut *state.posts;
        posts.loading = false;
        match page {
            Ok(page) => {
                let received = u32::try_from(page.posts.len()).unwrap_or(u32::MAX);
                posts.received = posts.received.saturating_add(received);
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

    pub(crate) fn sort_posts(&mut self, sort: PostSort) {
        self.change_forum_view(|view| view.sort = Some(sort));
    }

    pub(crate) fn lay_out_posts(&mut self, layout: PostLayout) {
        self.change_forum_view(|view| view.layout = Some(layout));
    }

    pub(crate) fn reset_forum_view(&mut self) {
        self.change_forum_view(|view| *view = ForumView::default());
    }

    fn change_forum_view(&mut self, change: impl FnOnce(&mut ForumView)) {
        let Some(((key, forum), sort)) = self
            .shown_forum()
            .map(|(seat, settings)| (seat, settings.sort))
        else {
            return;
        };
        let state = &mut self.conversation_mut().forum;
        let before = *state.view;
        let mut after = before;
        change(&mut after);
        if after == before {
            return;
        }
        state.view.set(after);
        self.remember_forum_view(&key, &forum, after);
        if after.sort.unwrap_or(sort) != before.sort.unwrap_or(sort) {
            self.reload_posts();
        }
    }

    pub(crate) fn filter_posts(&mut self, tag: TagId) {
        let filter = &mut self.conversation_mut().forum.filter;
        match filter.iter().position(|chosen| *chosen == tag) {
            Some(at) => {
                filter.remove(at);
            }
            None => filter.push(tag),
        }
        self.reload_posts();
    }

    pub(crate) fn search_typed(&mut self) {
        let state = &mut self.conversation_mut().forum;
        let typed = !state.search.text().trim().is_empty();
        state.typed.set(typed);
        after_keyed(SEARCH_TIMER, SEARCH_SETTLE, |chat: &mut Chat| {
            chat.search_posts()
        });
    }

    pub(crate) fn search_now(&mut self) {
        cancel(SEARCH_TIMER);
        self.search_posts();
    }

    pub(crate) fn clear_search(&mut self) {
        let state = &mut self.conversation_mut().forum;
        state.search.clear();
        state.typed.set(false);
        self.search_now();
    }

    fn search_posts(&mut self) {
        let state = &mut self.conversation_mut().forum;
        let wanted = state.search.text().trim().to_owned();
        if *state.searched == wanted {
            return;
        }
        state.searched.set(wanted);
        self.reload_posts();
    }
}

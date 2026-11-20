mod confirm;
mod menu;
mod pins;
mod threads;

use std::sync::Arc;

use discord_core::{Channel, Message, MessageId};
use rikka_app::prelude::*;
use rikka_ui::Element;

use crate::chat::Chat;
use crate::conversation::emoji;
use crate::conversation::lightbox::{Gallery, Shot};
use rikka_emoji::Group;

pub(crate) const MARK: &str = "discord-menu-mark";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    Emoji,
    Stickers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PickerTarget {
    Composer,
    React(MessageId),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OverlayAnchor {
    At(Point),
    Below(MessageId),
    Composer,
}

impl OverlayAnchor {
    pub(crate) fn anchor(&self) -> Anchor {
        match self {
            OverlayAnchor::At(at) => Anchor::new(MARK, AnchorEdge::BesideTop).offset(at.x, at.y),
            OverlayAnchor::Below(message) => {
                Anchor::new(row_key(message), AnchorEdge::BelowEnd).offset(-Space::ROOMY.px(), 0.0)
            }
            OverlayAnchor::Composer => Anchor::new(
                crate::conversation::composer::EMOJI_BUTTON,
                AnchorEdge::AboveEnd,
            )
            .offset(0.0, -Space::SNUG.px()),
        }
    }
}

pub(crate) fn row_key(message: &MessageId) -> (&'static str, MessageId) {
    ("discord-row", *message)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Shown {
    Menu {
        message: MessageId,
        anchor: OverlayAnchor,
        shift: bool,
        picture: Option<Shot>,
    },
    Picker {
        picker_target: PickerTarget,
        tab: Tab,
        anchor: OverlayAnchor,
    },
    Pins,
    Threads,
    NewThread {
        from: Option<MessageId>,
    },
    Viewer(Gallery),
    Delete(MessageId),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Listing<T> {
    Loading,
    Ready(Vec<T>),
    Failed(String),
}

#[derive(State)]
pub(crate) struct Overlay {
    #[untracked]
    menu_target: Option<(MessageId, bool)>,
    #[untracked]
    menu_picture: Option<Shot>,
    #[untracked]
    query: TextDocument,
    #[untracked]
    thread_name: TextDocument,
    shown: Tracked<Option<Shown>>,
    pins: Tracked<Listing<Arc<Message>>>,
    threads: Tracked<Listing<Channel>>,
    archived: Tracked<bool>,
    searched: Tracked<String>,
    creating: Tracked<bool>,
    #[untracked]
    recents_of: Option<String>,
    recents: Tracked<Vec<String>>,
    jumped: Tracked<Option<Group>>,
    zoomed: Tracked<bool>,
}

impl Overlay {
    pub(crate) fn new() -> Overlay {
        Overlay {
            menu_target: None,
            menu_picture: None,
            query: TextDocument::new(""),
            thread_name: TextDocument::new(""),
            shown: Tracked::new(None),
            pins: Tracked::new(Listing::Loading),
            threads: Tracked::new(Listing::Loading),
            archived: Tracked::new(false),
            searched: Tracked::new(String::new()),
            creating: Tracked::new(false),
            recents_of: None,
            recents: Tracked::new(Vec::new()),
            jumped: Tracked::new(None),
            zoomed: Tracked::new(false),
        }
    }

    pub(crate) fn shown(&self) -> Option<&Shown> {
        self.shown.as_ref()
    }

    pub(crate) fn recents(&self) -> &[String] {
        &self.recents
    }

    pub(crate) fn remember(&mut self, recents: Vec<String>) {
        self.recents.set(recents);
    }

    pub(crate) fn recents_of(&self) -> Option<&str> {
        self.recents_of.as_deref()
    }

    pub(crate) fn recall(&mut self, key: &str, recents: Vec<String>) {
        self.recents_of = Some(key.to_owned());
        self.recents.set(recents);
    }

    pub(crate) fn close(&mut self) {
        self.menu_target = None;
        self.menu_picture = None;
        if self.shown.is_some() {
            self.shown.set(None);
        }
        self.creating.set(false);
        self.zoom(false);
    }

    pub(crate) fn set_menu_target(&mut self, message: MessageId, shift: bool) {
        self.menu_target = Some((message, shift));
    }

    pub(crate) fn set_menu_picture(&mut self, picture: Shot) {
        self.menu_picture = Some(picture);
    }

    pub(crate) fn close_menu(&mut self) {
        if matches!(*self.shown, Some(Shown::Menu { .. })) {
            self.shown.set(None);
        }
    }

    pub(crate) fn open_menu_at(&mut self, at: Point) {
        let picture = self.menu_picture.take();
        let Some((message, shift)) = self.menu_target.take() else {
            return;
        };
        self.shown.set(Some(Shown::Menu {
            message,
            anchor: OverlayAnchor::At(at),
            shift,
            picture,
        }));
    }

    pub(crate) fn open_menu_below(&mut self, message: MessageId, shift: bool) {
        self.menu_picture = None;
        self.shown.set(Some(Shown::Menu {
            message,
            anchor: OverlayAnchor::Below(message),
            shift,
            picture: None,
        }));
    }

    pub(crate) fn open_picker(&mut self, picker_target: PickerTarget, tab: Tab) {
        let anchor = match (&picker_target, self.shown.as_ref()) {
            (PickerTarget::Composer, _) => OverlayAnchor::Composer,
            (PickerTarget::React(_), Some(Shown::Menu { anchor, .. })) => anchor.clone(),
            (PickerTarget::React(message), _) => OverlayAnchor::Below(*message),
        };
        self.query.clear();
        self.searched.set(String::new());
        self.jumped.set(None);
        self.shown.set(Some(Shown::Picker {
            picker_target,
            tab,
            anchor,
        }));
    }

    pub(crate) fn switch_tab(&mut self, tab: Tab) {
        self.jumped.set(None);
        if let Some(Shown::Picker { tab: shown, .. }) = self.shown.as_mut() {
            *shown = tab;
        }
    }

    pub(crate) fn searched(&mut self) {
        let query = self.query.text().trim().to_lowercase();
        self.jumped.set(None);
        self.searched.set(query);
    }

    pub(crate) fn search(&self) -> &str {
        &self.searched
    }

    pub(crate) fn jump(&mut self, group: Group) {
        self.jumped.set(Some(group));
    }

    pub(crate) fn jumped(&self) -> Option<Group> {
        *self.jumped
    }

    pub(crate) fn query(&self) -> &TextDocument {
        &self.query
    }

    pub(crate) fn open_gallery(&mut self, gallery: Gallery) {
        self.shown.set(Some(Shown::Viewer(gallery)));
        self.zoom(false);
    }

    pub(crate) fn zoomed(&self) -> bool {
        *self.zoomed
    }

    pub(crate) fn zoom(&mut self, zoomed: bool) {
        if *self.zoomed != zoomed {
            self.zoomed.set(zoomed);
        }
    }

    pub(crate) fn gallery(&self) -> Option<&Gallery> {
        match self.shown.as_ref() {
            Some(Shown::Viewer(gallery)) => Some(gallery),
            _ => None,
        }
    }

    pub(crate) fn show_shot(&mut self, at: usize) {
        let shown = self.gallery().and_then(|gallery| gallery.showing(at));
        self.show_gallery(shown);
    }

    pub(crate) fn step_gallery(&mut self, forward: bool) {
        let shown = self.gallery().and_then(|gallery| gallery.stepped(forward));
        self.show_gallery(shown);
    }

    fn show_gallery(&mut self, shown: Option<Gallery>) {
        match shown {
            Some(gallery) if self.gallery() != Some(&gallery) => self.open_gallery(gallery),
            _ => skip_view(),
        }
    }

    pub(crate) fn ask_new_thread(&mut self, from: Option<MessageId>, name: &str) {
        self.thread_name.set_text(name);
        self.creating.set(false);
        self.shown.set(Some(Shown::NewThread { from }));
    }

    pub(crate) fn thread_name(&self) -> &TextDocument {
        &self.thread_name
    }

    pub(crate) fn creating(&self) -> bool {
        *self.creating
    }

    pub(crate) fn ask_delete(&mut self, message: MessageId) {
        self.shown.set(Some(Shown::Delete(message)));
    }

    pub(crate) fn pins(&self) -> &Listing<Arc<Message>> {
        &self.pins
    }

    pub(crate) fn threads(&self) -> &Listing<Channel> {
        &self.threads
    }

    pub(crate) fn archived(&self) -> bool {
        *self.archived
    }

    pub(crate) fn show_archived(&mut self, archived: bool) {
        self.archived.set(archived);
    }
}

impl std::fmt::Debug for Overlay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Overlay")
            .field("shown", &*self.shown)
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(chat: &Chat) -> Element {
    let overlay = chat.conversation().overlay();
    let shown = match overlay.shown() {
        Some(shown) => shown.clone(),
        None => return spacer().size(Length::px(0.0), Length::px(0.0)).any(),
    };
    let layer = match &shown {
        Shown::Menu {
            message,
            anchor,
            shift,
            picture,
        } => menu::view(chat, message, anchor, *shift, picture.as_ref()),
        Shown::Picker {
            picker_target,
            tab,
            anchor,
        } => Some(emoji::picker(chat, picker_target, *tab, anchor)),
        Shown::Pins => Some(pins::view(chat)),
        Shown::Threads => Some(threads::view(chat)),
        Shown::NewThread { from } => Some(threads::naming(chat, from.as_ref())),
        Shown::Viewer(_) => None,
        Shown::Delete(message) => confirm::view(chat, message),
    };
    match layer {
        Some(layer) => column_of([layer])
            .size(Length::px(0.0), Length::px(0.0))
            .any(),
        None => spacer().size(Length::px(0.0), Length::px(0.0)).any(),
    }
}

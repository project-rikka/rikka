mod blend;
mod playlist;
mod rows;
mod track;

use std::hash::Hash;

use music_blend::{BlendId, EntryKey};
use music_core::{Id, Playlist, Track};
use rikka_app::prelude::*;
use rikka_ui::{Element, Handler};

use crate::music::Music;
use crate::widgets::aimed;
use rows::Rows;

pub(crate) const MARK: &str = "music-menu-mark";
const MAIN: &str = "music-menu";
const BRANCH: &str = "music-menu-branch";
const WIDTH: f32 = 256.0;
const BRANCH_WIDTH: f32 = 240.0;
const OVERLAP: f32 = -4.0;
const BELOW: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Subject {
    Track(TrackTarget),
    Playlist(Playlist),
    Blend(BlendId),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrackTarget {
    pub(crate) track: Track,
    pub(crate) context: TrackContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TrackContext {
    Listed,
    Playlist(Id),
    Blend(BlendId, EntryKey),
    Upcoming(usize),
    Playing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Branch {
    Playlists,
    Artists,
}

#[derive(Clone, Debug, PartialEq)]
struct Shown {
    subject: Subject,
    anchor: Anchor,
    branch: Option<(Branch, f32)>,
    key: u64,
}

#[derive(State)]
pub(crate) struct Menus {
    #[untracked]
    aimed: Option<Subject>,
    #[untracked]
    generation: u64,
    shown: Tracked<Option<Shown>>,
}

impl Menus {
    pub(crate) fn new() -> Menus {
        Menus {
            aimed: None,
            generation: 0,
            shown: Tracked::new(None),
        }
    }

    pub(crate) fn subject(&self) -> Option<&Subject> {
        self.shown.as_ref().map(|shown| &shown.subject)
    }

    pub(crate) fn branch(&self) -> Option<Branch> {
        self.shown
            .as_ref()
            .and_then(|shown| shown.branch)
            .map(|(branch, _)| branch)
    }

    pub(crate) fn aim(&mut self, subject: Subject) {
        self.aimed = Some(subject);
    }

    pub(crate) fn open_at(&mut self, at: Point) {
        let Some(subject) = self.aimed.take() else {
            return;
        };
        let anchor = Anchor::new(MARK, AnchorEdge::BesideTop).offset(at.x, at.y);
        self.show(subject, anchor);
    }

    pub(crate) fn open_below(&mut self, subject: Subject, button: impl Hash) {
        self.show(subject, Anchor::below(button).offset(0.0, BELOW));
    }

    pub(crate) fn open_branch(&mut self, branch: Branch, at: f32) {
        if let Some(shown) = self.shown.as_mut() {
            shown.branch = Some((branch, at));
        }
    }

    pub(crate) fn close_branch(&mut self) {
        if self.branch().is_some()
            && let Some(shown) = self.shown.as_mut()
        {
            shown.branch = None;
        }
    }

    pub(crate) fn close(&mut self) {
        self.aimed = None;
        if self.shown.is_some() {
            self.shown.set(None);
        }
    }

    fn show(&mut self, subject: Subject, anchor: Anchor) {
        self.generation += 1;
        self.shown.set(Some(Shown {
            subject,
            anchor,
            branch: None,
            key: self.generation,
        }));
    }
}

impl std::fmt::Debug for Menus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Menus")
            .field("shown", &self.subject())
            .finish_non_exhaustive()
    }
}

pub(crate) fn target(subject: Subject, body: impl View) -> Element {
    aimed(
        Handler::new(move |menus: &mut Menus| menus.aim(subject.clone())),
        body,
    )
}

pub(crate) fn more(subject: Subject, key: impl Hash + Clone + 'static) -> Element {
    let anchor = key.clone();
    button(
        icon(MaterialSymbol::MORE_HORIZ)
            .extent(Length::px(18.0))
            .color(palette().text_muted),
    )
    .ghost()
    .icon_only()
    .small()
    .on_press(move |menus: &mut Menus| menus.open_below(subject.clone(), anchor.clone()))
    .build()
    .keyed(key)
}

pub(crate) fn view(music: &Music) -> Vec<Element> {
    let Some(shown) = music.menus().shown.as_ref() else {
        return Vec::new();
    };
    let rows = match &shown.subject {
        Subject::Track(track_target) => track::rows(music, track_target),
        Subject::Playlist(playlist) => playlist::rows(music, playlist),
        Subject::Blend(id) => blend::rows(music, id),
    };
    let mut panels = vec![main(rows, shown)];
    if let Some((branch, at)) = shown.branch {
        let rows = match (&shown.subject, branch) {
            (Subject::Track(track_target), Branch::Playlists) => {
                track::playlists(music, track_target)
            }
            (Subject::Track(track_target), Branch::Artists) => track::artists(track_target),
            (Subject::Playlist(_) | Subject::Blend(_), _) => Rows::default(),
        };
        panels.push(branched(rows, shown.key, branch, at));
    }
    panels
}

fn main(rows: Rows, shown: &Shown) -> Element {
    let (items, branches) = rows.finish();
    let opening = branches.clone();
    menu()
        .items(items)
        .width(WIDTH)
        .on_activate(move |menus: &mut Menus, index: usize| {
            if branches.get(index).copied().flatten().is_none() {
                menus.close();
            }
        })
        .on_highlight(|menus: &mut Menus, _: Option<usize>| menus.close_branch())
        .on_open(move |menus: &mut Menus, index: usize, row: Rect| {
            if let Some(branch) = opening.get(index).copied().flatten() {
                menus.open_branch(branch, row.y());
            }
        })
        .on_dismiss(|menus: &mut Menus| menus.close())
        .build()
        .keyed((MAIN, shown.key))
        .anchored(shown.anchor)
}

fn branched(rows: Rows, key: u64, branch: Branch, at: f32) -> Element {
    let (items, _) = rows.finish();
    menu()
        .items(items)
        .width(BRANCH_WIDTH)
        .on_activate(|menus: &mut Menus, _: usize| menus.close())
        .on_dismiss(|menus: &mut Menus| menus.close_branch())
        .build()
        .keyed((BRANCH, key, branch))
        .anchored(Anchor::beside((MAIN, key)).offset(OVERLAP, at))
}

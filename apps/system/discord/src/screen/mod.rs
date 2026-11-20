pub(crate) mod golive;
mod stage;

use std::collections::BTreeSet;

use discord_core::{StreamKey, UserId};
use discord_stream::{Broadcaster, Quality, Watcher};
use rikka_app::prelude::*;
use rikka_audio::{AudioApp, AudioCapture};
use rikka_screencast::Cast;
use rikka_ui::Feed;

pub(crate) use golive::view as golive_view;
pub(crate) use stage::view as stage_view;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Choosing,
    Starting,
    Live,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sharing {
    pub(crate) account: String,
    pub(crate) key: Option<StreamKey>,
    pub(crate) phase: Phase,
    pub(crate) viewers: Vec<UserId>,
    pub(crate) paused: bool,
    pub(crate) watched: bool,
    pub(crate) size: Option<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Viewing {
    pub(crate) account: String,
    pub(crate) key: StreamKey,
    pub(crate) phase: Phase,
    pub(crate) viewers: Vec<UserId>,
    pub(crate) paused: bool,
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) volume: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum SoundPick {
    #[default]
    Silent,
    Everything,
    Apps(BTreeSet<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Setup {
    pub(crate) quality: Quality,
    pub(crate) sound: SoundPick,
    pub(crate) apps: Option<Vec<AudioApp>>,
}

#[derive(State)]
pub(crate) struct Screens {
    sharing: Tracked<Option<Sharing>>,
    viewing: Tracked<Option<Viewing>>,
    setup: Tracked<Option<Setup>>,
    stage: Tracked<bool>,
    #[untracked]
    pending_watch: Option<(String, StreamKey)>,
    #[untracked]
    captured: Option<(Cast, Option<AudioCapture>)>,
    #[untracked]
    broadcaster: Option<Broadcaster>,
    #[untracked]
    watcher: Option<Watcher>,
    #[untracked]
    picture: Feed,
    #[untracked]
    thumbnail: Feed,
}

impl Screens {
    pub(crate) fn new() -> Screens {
        Screens {
            sharing: Tracked::new(None),
            viewing: Tracked::new(None),
            setup: Tracked::new(None),
            stage: Tracked::new(false),
            pending_watch: None,
            captured: None,
            broadcaster: None,
            watcher: None,
            picture: Feed::new(),
            thumbnail: Feed::new(),
        }
    }

    pub(crate) fn sharing(&self) -> Option<&Sharing> {
        self.sharing.as_ref()
    }

    pub(crate) fn viewing(&self) -> Option<&Viewing> {
        self.viewing.as_ref()
    }

    pub(crate) fn setup(&self) -> Option<&Setup> {
        self.setup.as_ref()
    }

    pub(crate) fn stage_shown(&self) -> bool {
        *self.stage && (self.viewing.is_some() || self.sharing.is_some())
    }

    pub(crate) fn picture(&self) -> Feed {
        self.picture.clone()
    }

    pub(crate) fn thumbnail(&self) -> Feed {
        self.thumbnail.clone()
    }

    pub(crate) fn watches(&self, key: &StreamKey) -> bool {
        self.viewing
            .as_ref()
            .is_some_and(|viewing| viewing.key == *key)
    }

    pub(crate) fn set_stage(&mut self, shown: bool) {
        self.stage.set(shown);
    }

    pub(crate) fn begin_setup(&mut self, setup: Setup) {
        self.setup.set(Some(setup));
    }

    pub(crate) fn change_setup(&mut self, change: impl FnOnce(&mut Setup)) {
        changed(&mut self.setup, change);
    }

    pub(crate) fn end_setup(&mut self) -> Option<Setup> {
        self.setup.take()
    }

    pub(crate) fn begin_sharing(&mut self, account: String) {
        self.sharing.set(Some(Sharing {
            account,
            key: None,
            phase: Phase::Choosing,
            viewers: Vec::new(),
            paused: false,
            watched: true,
            size: None,
        }));
    }

    pub(crate) fn change_sharing(&mut self, change: impl FnOnce(&mut Sharing)) {
        changed(&mut self.sharing, change);
    }

    pub(crate) fn store_capture(&mut self, cast: Cast, sound: Option<AudioCapture>) {
        self.captured = Some((cast, sound));
    }

    pub(crate) fn take_captured(&mut self) -> Option<(Cast, Option<AudioCapture>)> {
        self.captured.take()
    }

    pub(crate) fn adopt_broadcaster(&mut self, broadcaster: Broadcaster) {
        self.broadcaster = Some(broadcaster);
    }

    pub(crate) fn broadcaster(&self) -> Option<&Broadcaster> {
        self.broadcaster.as_ref()
    }

    pub(crate) fn stop_sharing(&mut self) -> Option<Broadcaster> {
        self.sharing.set(None);
        self.captured = None;
        self.broadcaster.take()
    }

    pub(crate) fn begin_viewing(&mut self, account: String, key: StreamKey) {
        self.viewing.set(Some(Viewing {
            account,
            key,
            phase: Phase::Starting,
            viewers: Vec::new(),
            paused: false,
            size: None,
            volume: 1.0,
        }));
        self.stage.set(true);
        self.picture = Feed::new();
    }

    pub(crate) fn change_viewing(&mut self, change: impl FnOnce(&mut Viewing)) {
        changed(&mut self.viewing, change);
    }

    pub(crate) fn adopt_watcher(&mut self, watcher: Watcher) {
        self.watcher = Some(watcher);
    }

    pub(crate) fn watcher(&self) -> Option<&Watcher> {
        self.watcher.as_ref()
    }

    pub(crate) fn stop_viewing(&mut self) -> Option<Watcher> {
        self.viewing.set(None);
        self.watcher.take()
    }

    pub(crate) fn wait_to_watch(&mut self, account: String, key: StreamKey) {
        self.pending_watch = Some((account, key));
    }

    pub(crate) fn take_pending_watch(&mut self) -> Option<(String, StreamKey)> {
        self.pending_watch.take()
    }

    pub(crate) fn forget_pending_watch(&mut self) {
        self.pending_watch = None;
    }
}

fn changed<T: Clone + PartialEq>(tracked: &mut Tracked<Option<T>>, change: impl FnOnce(&mut T)) {
    let Some(mut value) = tracked.as_ref().cloned() else {
        return;
    };
    change(&mut value);
    tracked.set(Some(value));
}

impl std::fmt::Debug for Screens {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Screens")
            .field("sharing", &*self.sharing)
            .field("viewing", &*self.viewing)
            .finish_non_exhaustive()
    }
}

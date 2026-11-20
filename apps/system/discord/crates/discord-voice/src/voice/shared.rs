use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use discord_core::UserId;
use rikka_sync::Mutex;
use rikka_worker::{EventFd, EventNotifier, Stop, event_fd};

use crate::settings::{VoiceSettings, volume};
use crate::voice::attached::Attached;

pub(crate) struct Shared {
    muted: AtomicBool,
    deafened: AtomicBool,
    push_to_talk: AtomicBool,
    stop: AtomicBool,
    settings: Mutex<VoiceSettings>,
    revision: AtomicU64,
    user_mix: Mutex<HashMap<UserId, UserMix>>,
    user_mix_revision: AtomicU64,
    attached: Attached,
    receive_bell: EventNotifier,
    send_bell: EventNotifier,
}

pub(crate) struct Bells {
    pub(crate) receive: EventFd,
    pub(crate) send: EventFd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UserMix {
    volume: f32,
    muted: bool,
}

impl Default for UserMix {
    fn default() -> UserMix {
        UserMix {
            volume: 1.0,
            muted: false,
        }
    }
}

impl UserMix {
    pub(crate) fn gain(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }
}

impl Shared {
    pub(crate) fn new(
        settings: VoiceSettings,
        muted: bool,
        deafened: bool,
    ) -> std::io::Result<(Shared, Bells)> {
        let (receive_bell, receive) = event_fd()?;
        let (send_bell, send) = event_fd()?;
        let shared = Shared {
            muted: AtomicBool::new(muted),
            deafened: AtomicBool::new(deafened),
            push_to_talk: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            settings: Mutex::new(settings),
            revision: AtomicU64::new(1),
            user_mix: Mutex::new(HashMap::new()),
            user_mix_revision: AtomicU64::new(1),
            attached: Attached::new(receive_bell.clone()),
            receive_bell,
            send_bell,
        };
        Ok((shared, Bells { receive, send }))
    }

    pub(crate) fn receive_bell(&self) -> &EventNotifier {
        &self.receive_bell
    }

    pub(crate) fn send_bell(&self) -> &EventNotifier {
        &self.send_bell
    }

    fn ring(&self) {
        if let Err(error) = self.receive_bell.notify() {
            tracing::warn!(%error, "failed to wake the voice receiver");
        }
    }

    fn ring_both(&self) {
        self.ring();
        if let Err(error) = self.send_bell.notify() {
            tracing::warn!(%error, "failed to wake the voice sender");
        }
    }

    pub(crate) fn attached(&self) -> &Attached {
        &self.attached
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub(crate) fn set_deafened(&self, deafened: bool) {
        self.deafened.store(deafened, Ordering::Relaxed);
        self.ring();
    }

    pub(crate) fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub(crate) fn deafened(&self) -> bool {
        self.deafened.load(Ordering::Relaxed)
    }

    pub(crate) fn silenced(&self) -> bool {
        self.muted() || self.deafened()
    }

    pub(crate) fn set_push_to_talk(&self, pressed: bool) {
        self.push_to_talk.store(pressed, Ordering::Relaxed);
    }

    pub(crate) fn push_to_talk_pressed(&self) -> bool {
        self.push_to_talk.load(Ordering::Relaxed)
    }

    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.ring_both();
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub(crate) fn set_settings(&self, settings: VoiceSettings) {
        *self.settings.lock() = settings;
        self.revision.fetch_add(1, Ordering::Release);
        self.ring_both();
    }

    pub(crate) fn settings_since(&self, known: u64) -> Option<(u64, VoiceSettings)> {
        let revision = self.revision.load(Ordering::Acquire);
        (revision != known).then(|| (revision, self.settings.lock().clone()))
    }

    pub(crate) fn set_user_volume(&self, user: &UserId, level: f32) {
        self.change_user_mix(user, |user_mix| user_mix.volume = volume(level));
    }

    pub(crate) fn set_user_muted(&self, user: &UserId, muted: bool) {
        self.change_user_mix(user, |user_mix| user_mix.muted = muted);
    }

    pub(crate) fn user_mix_since(&self, known: u64) -> Option<(u64, HashMap<UserId, UserMix>)> {
        let revision = self.user_mix_revision.load(Ordering::Acquire);
        (revision != known).then(|| (revision, self.user_mix.lock().clone()))
    }

    fn change_user_mix(&self, user: &UserId, change: impl FnOnce(&mut UserMix)) {
        change(self.user_mix.lock().entry(*user).or_default());
        self.user_mix_revision.fetch_add(1, Ordering::Release);
        self.ring();
    }
}

impl Stop for Shared {
    fn stop(&self) {
        Shared::stop(self);
    }
}

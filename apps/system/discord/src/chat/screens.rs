use discord_client::{Broadcast, Watch};
use discord_core::{StreamKey, UserId};
use discord_stream::{Broadcaster, FrameRate, Resolution, StreamUpdate, Watcher};
use rikka_app::prelude::*;
use rikka_audio::{AudioApp, AudioCapture, CaptureConfig, CaptureSource};
use rikka_opus::OPUS_SAMPLE_RATE;
use rikka_screencast::{Cast, ScreenCast};

use super::Chat;
use crate::screen::{Phase, Screens, Setup, SoundPick};

const SHARED_CHANNELS: u16 = 2;
const LISTING: &str = "discord-screen-apps";
const CAPTURING: &str = "discord-screen-capture";
const GOING_LIVE: &str = "discord-screen-go-live";
const SHARED_UPDATES: &str = "discord-screen-shared";
const WATCHING: &str = "discord-screen-watch";
const WATCHED_UPDATES: &str = "discord-screen-watched";
const PREVIEW: &str = "discord-screen-preview";

type Captured = Result<(Cast, Option<AudioCapture>), rikka_screencast::Error>;

impl Chat {
    pub(crate) fn screens(&self) -> &Screens {
        &self.screens
    }

    pub(crate) fn can_share_screen(&self) -> bool {
        self.calls.seat().is_some() && self.screens.sharing().is_none()
    }

    pub(crate) fn open_go_live(&mut self) {
        if !self.can_share_screen() {
            return;
        }
        let sound = match self.preferences.stream_sound() {
            true => SoundPick::Everything,
            false => SoundPick::Silent,
        };
        self.screens.begin_setup(Setup {
            quality: self.preferences.stream_quality(),
            sound,
            apps: None,
        });
        spawn_blocking_keyed(
            LISTING,
            || AudioCapture::apps().unwrap_or_default(),
            |chat: &mut Chat, apps: Vec<AudioApp>| {
                chat.screens.change_setup(|setup| setup.apps = Some(apps));
            },
        );
    }

    pub(crate) fn close_go_live(&mut self) {
        cancel(LISTING);
        self.screens.end_setup();
    }

    pub(crate) fn choose_resolution(&mut self, resolution: Resolution) {
        self.screens
            .change_setup(|setup| setup.quality.resolution = resolution);
    }

    pub(crate) fn choose_frame_rate(&mut self, frame_rate: FrameRate) {
        self.screens
            .change_setup(|setup| setup.quality.frame_rate = frame_rate);
    }

    pub(crate) fn choose_sound(&mut self, sound: SoundPick) {
        self.screens.change_setup(|setup| setup.sound = sound);
    }

    pub(crate) fn toggle_shared_app(&mut self, key: &str, chosen: bool) {
        self.screens.change_setup(|setup| {
            let mut apps = match std::mem::take(&mut setup.sound) {
                SoundPick::Apps(apps) => apps,
                SoundPick::Silent | SoundPick::Everything => Default::default(),
            };
            match chosen {
                true => apps.insert(key.to_owned()),
                false => apps.remove(key),
            };
            setup.sound = SoundPick::Apps(apps);
        });
    }

    pub(crate) fn go_live(&mut self) {
        let Some(seat) = self.calls.seat().cloned() else {
            return;
        };
        let Some(setup) = self.screens.end_setup() else {
            return;
        };
        cancel(LISTING);
        self.preferences.set_stream_quality(setup.quality);
        self.preferences
            .set_stream_sound(setup.sound == SoundPick::Everything);
        self.screens.begin_sharing(seat.key.clone());
        let frame_rate = setup.quality.frame_rate.per_second();
        let sound = setup.sound.clone();
        spawn_blocking_keyed(
            CAPTURING,
            move || capture(frame_rate, &sound),
            move |chat: &mut Chat, captured: Captured| chat.screen_captured(captured),
        );
    }

    fn screen_captured(&mut self, captured: Captured) {
        let (cast, sound) = match captured {
            Ok(captured) => captured,
            Err(error) => {
                self.stop_sharing();
                if error != rikka_screencast::Error::Cancelled {
                    self.error_message.set(Some(error.to_string()));
                }
                return;
            }
        };
        let Some(seat) = self.calls.seat().cloned() else {
            self.stop_sharing();
            return;
        };
        let Some(client) = self.linked(&seat.key).map(|linked| linked.client().clone()) else {
            self.stop_sharing();
            return;
        };
        self.screens.store_capture(cast, sound);
        self.screens
            .change_sharing(|sharing| sharing.phase = Phase::Starting);
        let channel = seat.channel;
        spawn_keyed(
            GOING_LIVE,
            async move { client.go_live(&channel).await },
            |chat: &mut Chat, broadcast: discord_core::Result<Broadcast>| {
                chat.on_go_live(broadcast);
            },
        );
    }

    fn on_go_live(&mut self, broadcast: discord_core::Result<Broadcast>) {
        let broadcast = match broadcast {
            Ok(broadcast) => broadcast,
            Err(error) => {
                self.stop_sharing();
                self.error_message.set(Some(error.to_string()));
                return;
            }
        };
        let Some((cast, sound)) = self.screens.take_captured() else {
            broadcast.line.end();
            return;
        };
        let key = broadcast.key;
        let quality = self.preferences.stream_quality();
        match Broadcaster::start(broadcast, cast, sound, quality, {
            let thumbnail = self.screens.thumbnail();
            move |width, height, rgba| thumbnail.show(width, height, rgba)
        }) {
            Ok(mut broadcaster) => {
                let updates = broadcaster.updates();
                self.screens.adopt_broadcaster(broadcaster);
                self.screens.change_sharing(|sharing| {
                    sharing.key = Some(key);
                    sharing.phase = Phase::Live;
                });
                stream_keyed(SHARED_UPDATES, updates, |chat: &mut Chat, update| {
                    chat.handle_broadcast_update(update);
                });
            }
            Err(error) => {
                self.stop_sharing();
                self.error_message.set(Some(error.to_string()));
            }
        }
    }

    pub(crate) fn handle_broadcast_update(&mut self, update: StreamUpdate) {
        match update {
            StreamUpdate::Viewers(viewers) => {
                let me = self.sharing_me();
                self.screens.change_sharing(|sharing| {
                    sharing.viewers = viewers
                        .into_iter()
                        .filter(|viewer| Some(*viewer) != me)
                        .collect();
                });
            }
            StreamUpdate::Watched(watched) => {
                self.screens
                    .change_sharing(|sharing| sharing.watched = watched);
            }
            StreamUpdate::Paused(paused) => {
                self.screens
                    .change_sharing(|sharing| sharing.paused = paused);
            }
            StreamUpdate::Showing { width, height } => {
                self.screens
                    .change_sharing(|sharing| sharing.size = Some((width, height)));
            }
            StreamUpdate::Preview(jpeg) => self.upload_preview(jpeg),
            StreamUpdate::Ended(reason) => {
                self.stop_sharing();
                if let Some(reason) = reason {
                    self.error_message.set(Some(reason));
                }
            }
        }
    }

    fn sharing_me(&self) -> Option<UserId> {
        let account = &self.screens.sharing()?.account;
        self.linked(account)?.me().map(|me| me.id)
    }

    fn upload_preview(&mut self, jpeg: Vec<u8>) {
        skip_view();
        let Some(sharing) = self.screens.sharing() else {
            return;
        };
        let (Some(key), Some(client)) = (
            sharing.key,
            self.linked(&sharing.account)
                .map(|linked| linked.client().clone()),
        ) else {
            return;
        };
        spawn_keyed(
            PREVIEW,
            async move { client.upload_preview(key, jpeg).await },
            |_: &mut Chat, uploaded: discord_core::Result<()>| {
                skip_view();
                if let Err(error) = uploaded {
                    tracing::debug!(%error, "failed to upload stream preview");
                }
            },
        );
    }

    pub(crate) fn pause_sharing(&mut self, paused: bool) {
        if let Some(broadcaster) = self.screens.broadcaster() {
            broadcaster.pause(paused);
        }
        self.screens
            .change_sharing(|sharing| sharing.paused = paused);
    }

    pub(crate) fn stop_sharing(&mut self) {
        for key in [CAPTURING, GOING_LIVE, SHARED_UPDATES, PREVIEW] {
            cancel(key);
        }
        if let Some(broadcaster) = self.screens.stop_sharing() {
            spawn_blocking(move || broadcaster.end(), |(): ()| skip_view());
        }
    }

    pub(crate) fn watch_stream(&mut self, account: &str, key: StreamKey) {
        let mine = self
            .linked(account)
            .is_some_and(|linked| linked.is_me(&key.owner));
        if mine {
            if self.screens.sharing().is_some() {
                self.screens.set_stage(true);
            }
            return;
        }
        if self.screens.watches(&key) {
            self.screens.set_stage(true);
            return;
        }
        if !self.calls.is_seated_in(account, &key.channel) {
            self.join_voice_on(account, &key.channel);
            self.screens.wait_to_watch(account.to_owned(), key);
            return;
        }
        if self.calls.phase() != crate::voice::Phase::Live {
            self.screens.wait_to_watch(account.to_owned(), key);
            return;
        }
        self.start_watching(account, key);
    }

    pub(crate) fn watch_when_seated(&mut self) {
        let Some((account, key)) = self.screens.take_pending_watch() else {
            return;
        };
        if self.calls.is_seated_in(&account, &key.channel) {
            self.start_watching(&account, key);
        }
    }

    fn start_watching(&mut self, account: &str, key: StreamKey) {
        let Some(client) = self.linked(account).map(|linked| linked.client().clone()) else {
            return;
        };
        self.stop_watching();
        self.screens.begin_viewing(account.to_owned(), key);
        spawn_keyed(
            WATCHING,
            async move { client.watch(key).await },
            move |chat: &mut Chat, watch: discord_core::Result<Watch>| {
                chat.started_watching(key, watch)
            },
        );
    }

    fn started_watching(&mut self, key: StreamKey, watch: discord_core::Result<Watch>) {
        if !self.screens.watches(&key) {
            if let Ok(watch) = watch {
                watch.line.leave();
            }
            return;
        }
        let watch = match watch {
            Ok(watch) => watch,
            Err(error) => {
                self.stop_watching();
                self.error_message.set(Some(error.to_string()));
                return;
            }
        };
        let Some(engine) = self.calls.engine() else {
            watch.line.leave();
            self.stop_watching();
            return;
        };
        let sound = engine.hear_stream();
        match Watcher::start(watch, sound, {
            let picture = self.screens.picture();
            move |width, height, rgba| picture.show(width, height, rgba)
        }) {
            Ok(mut watcher) => {
                let updates = watcher.updates();
                self.screens.adopt_watcher(watcher);
                self.screens
                    .change_viewing(|viewing| viewing.phase = Phase::Live);
                stream_keyed(WATCHED_UPDATES, updates, |chat: &mut Chat, update| {
                    chat.handle_watch_update(update);
                });
            }
            Err(error) => {
                self.stop_watching();
                self.error_message.set(Some(error.to_string()));
            }
        }
    }

    pub(crate) fn handle_watch_update(&mut self, update: StreamUpdate) {
        match update {
            StreamUpdate::Viewers(viewers) => {
                self.screens
                    .change_viewing(|viewing| viewing.viewers = viewers);
            }
            StreamUpdate::Paused(paused) => {
                self.screens
                    .change_viewing(|viewing| viewing.paused = paused);
            }
            StreamUpdate::Showing { width, height } => {
                self.screens
                    .change_viewing(|viewing| viewing.size = Some((width, height)));
            }
            StreamUpdate::Ended(reason) => {
                self.stop_watching();
                if let Some(reason) = reason {
                    self.error_message.set(Some(reason));
                }
            }
            StreamUpdate::Watched(_) | StreamUpdate::Preview(_) => skip_view(),
        }
    }

    pub(crate) fn set_stream_volume(&mut self, volume: f32) {
        if let Some(watcher) = self.screens.watcher() {
            watcher.set_volume(volume);
        }
        self.screens
            .change_viewing(|viewing| viewing.volume = volume);
    }

    pub(crate) fn stop_watching(&mut self) {
        for key in [WATCHING, WATCHED_UPDATES] {
            cancel(key);
        }
        self.screens.forget_pending_watch();
        if let Some(watcher) = self.screens.stop_viewing() {
            spawn_blocking(move || watcher.leave(), |(): ()| skip_view());
        }
    }

    pub(crate) fn show_stage(&mut self, shown: bool) {
        self.screens.set_stage(shown);
    }

    pub(crate) fn end_screens(&mut self) {
        self.close_go_live();
        self.stop_sharing();
        self.stop_watching();
    }
}

fn capture(frame_rate: u32, sound: &SoundPick) -> Captured {
    let cast = ScreenCast::new().frame_rate(frame_rate).start()?;
    let source = match sound {
        SoundPick::Silent => return Ok((cast, None)),
        SoundPick::Apps(apps) if apps.is_empty() => return Ok((cast, None)),
        SoundPick::Everything => CaptureSource::OtherApps,
        SoundPick::Apps(apps) => CaptureSource::Apps(apps.iter().cloned().collect()),
    };
    let config = CaptureConfig::default()
        .source(source)
        .sample_rate(OPUS_SAMPLE_RATE)
        .channels(SHARED_CHANNELS);
    match AudioCapture::start(config) {
        Ok(sound) => Ok((cast, Some(sound))),
        Err(error) => {
            tracing::warn!(%error, "sharing screen without sound");
            Ok((cast, None))
        }
    }
}

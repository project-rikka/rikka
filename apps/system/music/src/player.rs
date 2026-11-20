use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::future::join_all;
use music_core::{
    Catalog, Clock, Connection, Id, Playback, PlaybackEvent, Quality, Queue, Repeat, Service, Track,
};
use rikka_app::prelude::*;
use rikka_hifi::Hifi;
use rikka_mpris::Publisher;

use crate::blends::Known;
use crate::music::Start;

const DRIFT: Duration = Duration::from_millis(250);
const RESTART: Duration = Duration::from_secs(3);
const SEEK_STEP: Duration = Duration::from_secs(10);
const VOLUME_STEP: f32 = 0.1;
const LOUDNESS: f32 = 0.8;
const SUBSTITUTE: &str = "music-substitute";

#[derive(Clone)]
struct LinkedAccount {
    key: String,
    connection: Connection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Route {
    key: String,
    track: Track,
}

struct FallbackCatalog {
    key: String,
    service: Service,
    catalog: Arc<dyn Catalog>,
}

#[derive(State)]
pub(crate) struct Player {
    links: Tracked<Vec<LinkedAccount>>,
    browsed: Tracked<Option<String>>,
    #[untracked]
    order: Vec<String>,
    #[untracked]
    route: Option<Route>,
    #[untracked]
    known: Known,
    queue: Tracked<Queue>,
    now: Tracked<Option<Track>>,
    via: Tracked<Option<Service>>,
    clock: Tracked<Clock>,
    playing: Tracked<bool>,
    loading: Tracked<bool>,
    volume: Tracked<f32>,
    #[untracked]
    unmuted: f32,
    #[untracked]
    misses: usize,
    error_message: Tracked<Option<String>>,
    #[untracked]
    mpris: Option<Publisher>,
    #[untracked]
    output: Hifi,
    #[untracked]
    prepared: Option<Id>,
}

impl Player {
    pub(crate) fn new(order: Vec<String>, known: Known, output: Hifi) -> Player {
        output.set_volume(LOUDNESS);
        Player {
            links: Tracked::new(Vec::new()),
            browsed: Tracked::new(None),
            order,
            route: None,
            known,
            queue: Tracked::new(Queue::new(seed())),
            now: Tracked::new(None),
            via: Tracked::new(None),
            clock: Tracked::default(),
            playing: Tracked::new(false),
            loading: Tracked::new(false),
            volume: Tracked::new(LOUDNESS),
            unmuted: LOUDNESS,
            misses: 0,
            error_message: Tracked::new(None),
            mpris: None,
            output,
            prepared: None,
        }
    }

    pub(crate) fn link(&mut self, key: &str, connection: Connection, quality: Quality) {
        self.unlink(key);
        connection.playback.set_quality(quality);
        let account = key.to_owned();
        stream_keyed(
            events_key(key),
            connection.playback.events(),
            move |player: &mut Player, event: PlaybackEvent| {
                player.handle_event_from(&account, event)
            },
        );
        self.links.push(LinkedAccount {
            key: key.to_owned(),
            connection,
        });
    }

    pub(crate) fn unlink(&mut self, key: &str) {
        let Some(at) = self.links.iter().position(|linked| linked.key == key) else {
            return;
        };
        cancel(events_key(key));
        let removed = self.links.remove(at);
        if self.route.as_ref().is_some_and(|route| route.key == key) {
            removed.connection.playback.pause();
            self.route = None;
            self.stop();
        }
    }

    pub(crate) fn browse(&mut self, key: Option<String>) {
        self.browsed.set(key);
    }

    pub(crate) fn browsed(&self) -> Option<&str> {
        self.browsed.as_deref()
    }

    pub(crate) fn linked(&self, key: &str) -> Option<&Connection> {
        self.links
            .iter()
            .find(|linked| linked.key == key)
            .map(|linked| &linked.connection)
    }

    pub(crate) fn first_linked(&self) -> Option<String> {
        self.links.first().map(|linked| linked.key.clone())
    }

    pub(crate) fn connections(&self) -> Vec<Connection> {
        self.links
            .iter()
            .map(|linked| linked.connection.clone())
            .collect()
    }

    pub(crate) fn set_order(&mut self, order: Vec<String>) {
        self.order = order;
    }

    pub(crate) fn now(&self) -> Option<&Track> {
        self.now.as_ref()
    }

    pub(crate) fn via(&self) -> Option<Service> {
        *self.via
    }

    pub(crate) fn clock(&self) -> Clock {
        *self.clock
    }

    pub(crate) fn playing(&self) -> bool {
        *self.playing
    }

    pub(crate) fn loading(&self) -> bool {
        *self.loading
    }

    pub(crate) fn volume(&self) -> f32 {
        *self.volume
    }

    pub(crate) fn shuffle(&self) -> bool {
        self.queue.shuffle()
    }

    pub(crate) fn repeat(&self) -> Repeat {
        self.queue.repeat()
    }

    pub(crate) fn upcoming(&self) -> Arc<[Track]> {
        self.queue.upcoming().cloned().collect()
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn is_now(&self, track: &Track) -> bool {
        self.now.as_ref().is_some_and(|now| now.is(track))
    }

    pub(crate) fn play(&mut self, tracks: &[Track], from: usize) {
        if self.queue.start(tracks.to_vec(), from).is_some() {
            self.misses = 0;
            self.begin();
        }
    }

    pub(crate) fn shuffled(&mut self, tracks: &[Track]) {
        if tracks.is_empty() {
            return;
        }
        self.queue.set_shuffle(true);
        let from = (seed() % tracks.len() as u64) as usize;
        self.play(tracks, from);
    }

    pub(crate) fn start(&mut self, tracks: &[Track], start: Start) {
        match start {
            Start::Play => self.play(tracks, 0),
            Start::Shuffle => self.shuffled(tracks),
            Start::Queue => self.enqueue(tracks),
        }
    }

    pub(crate) fn toggle(&mut self) {
        let Some(playback) = self.playback() else {
            return;
        };
        let now = Instant::now();
        match *self.playing {
            true => {
                playback.pause();
                self.clock.pause(now);
                self.playing.set(false);
            }
            false => {
                playback.play();
                let at = self.clock.position(now);
                self.clock.run(at, now);
                self.playing.set(true);
            }
        }
        self.announce();
    }

    pub(crate) fn next(&mut self) {
        match self.queue.advance(false).is_some() {
            true => self.begin(),
            false => self.finish(),
        }
    }

    pub(crate) fn previous(&mut self) {
        if self.clock.position(Instant::now()) > RESTART || self.queue.back().is_none() {
            self.seek(Duration::ZERO);
            return;
        }
        self.begin();
    }

    pub(crate) fn skip_to(&mut self, ahead: usize) {
        if self.queue.skip_to(ahead).is_some() {
            self.begin();
        }
    }

    pub(crate) fn seek(&mut self, at: Duration) {
        let Some(playback) = self.playback() else {
            return;
        };
        playback.seek(at);
        let now = Instant::now();
        match *self.playing {
            true => self.clock.run(at, now),
            false => self.clock.stop(at),
        }
        self.announce();
    }

    pub(crate) fn seek_by(&mut self, ahead: bool) {
        let at = self.clock.position(Instant::now());
        let length = self.clock.length();
        let to = match ahead {
            true => (at + SEEK_STEP).min(length),
            false => at.saturating_sub(SEEK_STEP),
        };
        self.seek(to);
    }

    pub(crate) fn step_volume(&mut self, up: bool) {
        let step = if up { VOLUME_STEP } else { -VOLUME_STEP };
        self.set_volume(*self.volume + step);
    }

    pub(crate) fn seek_fraction(&mut self, fraction: f32) {
        let length = self.clock.length();
        self.seek(length.mul_f32(fraction.clamp(0.0, 1.0)));
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.volume.set(volume.clamp(0.0, 1.0));
        self.output.set_volume(*self.volume);
        self.announce();
    }

    pub(crate) fn toggle_mute(&mut self) {
        match *self.volume > 0.0 {
            true => {
                self.unmuted = *self.volume;
                self.set_volume(0.0);
            }
            false => self.set_volume(self.unmuted),
        }
    }

    pub(crate) fn toggle_shuffle(&mut self) {
        let shuffle = !self.queue.shuffle();
        self.queue.set_shuffle(shuffle);
        self.prepare();
        self.announce();
    }

    pub(crate) fn cycle_repeat(&mut self) {
        let repeat = self.queue.repeat().next();
        self.set_repeat(repeat);
    }

    pub(crate) fn set_repeat(&mut self, repeat: Repeat) {
        self.queue.set_repeat(repeat);
        self.prepare();
        self.announce();
    }

    pub(crate) fn enqueue(&mut self, tracks: &[Track]) {
        let idle = self.now.is_none();
        for track in tracks.iter().filter(|track| track.playable) {
            self.queue.enqueue(track.clone());
        }
        if idle {
            self.begin();
        }
        self.prepare();
    }

    pub(crate) fn play_next(&mut self, track: Track) {
        if !track.playable {
            return;
        }
        let idle = self.now.is_none();
        self.queue.play_next(track);
        if idle {
            self.begin();
        }
        self.prepare();
    }

    pub(crate) fn drop_upcoming(&mut self, ahead: usize, track: &Track) {
        let still = self
            .queue
            .upcoming()
            .nth(ahead)
            .is_some_and(|queued| queued.is(track));
        let at = match still {
            true => Some(ahead),
            false => self.queue.upcoming().position(|queued| queued.is(track)),
        };
        if let Some(at) = at {
            self.queue.remove_upcoming(at);
        }
        self.prepare();
    }

    pub(crate) fn set_quality(&mut self, quality: Quality) {
        for linked in self.links.iter() {
            linked.connection.playback.set_quality(quality);
        }
        let (Some(playback), Some(route)) = (self.playback(), self.route.clone()) else {
            return;
        };
        let now = Instant::now();
        let at = self.clock.position(now);
        self.clock.pause(now);
        playback.load(&route.track.id, at, *self.playing);
        self.loading.set(true);
        self.prepared = None;
        self.prepare();
    }

    pub(crate) fn dismiss(&mut self) {
        self.error_message.set(None);
    }

    pub(crate) fn has_next(&self) -> bool {
        self.queue.upcoming().next().is_some() || self.queue.repeat() == Repeat::All
    }

    pub(crate) fn publish_to(&mut self, publisher: Publisher) {
        self.mpris = Some(publisher);
        self.announce();
    }

    fn announce(&self) {
        if let Some(publisher) = &self.mpris {
            publisher.show(crate::mpris::playback(self));
        }
    }

    fn begin(&mut self) {
        let Some(track) = self.queue.current().cloned() else {
            return;
        };
        self.clock.set(Clock::new(track.duration));
        self.playing.set(true);
        self.loading.set(true);
        if self.via.is_some() {
            self.via.set(None);
        }
        self.now.set(Some(track.clone()));
        match self.native(&track) {
            Some(key) => self.sound(key, track),
            None => self.substitute(&track),
        }
        self.announce();
    }

    fn sound(&mut self, key: String, track: Track) {
        cancel(SUBSTITUTE);
        let Some(playback) = self.playback_of(&key) else {
            return;
        };
        let same = self.route.as_ref().is_some_and(|route| route.key == key);
        if !same && let Some(previous) = self.playback() {
            previous.pause();
        }
        playback.load(&track.id, Duration::ZERO, true);
        let via = (!self.is_now(&track)).then_some(track.service);
        if *self.via != via {
            self.via.set(via);
        }
        self.route = Some(Route { key, track });
        self.prepared = None;
        self.prepare();
    }

    fn prepare(&mut self) {
        let Some(route) = &self.route else {
            return;
        };
        let next = self
            .queue
            .following()
            .filter(|next| self.native(next).as_deref() == Some(route.key.as_str()))
            .map(|next| next.id.clone());
        if self.prepared == next {
            return;
        }
        let Some(playback) = self.playback_of(&route.key) else {
            return;
        };
        playback.prepare(next.as_ref());
        self.prepared = next;
    }

    fn advanced(&mut self, to: &Id, now: Instant) {
        let expected = self.queue.following().is_some_and(|next| next.id == *to);
        let Some(track) = self.queue.advance(true).cloned() else {
            self.finish();
            return;
        };
        if !expected {
            self.begin();
            return;
        }
        self.clock.set(Clock::new(track.duration));
        self.clock.run(Duration::ZERO, now);
        self.misses = 0;
        self.playing.set(true);
        self.loading.set(false);
        if self.via.is_some() {
            self.via.set(None);
        }
        self.now.set(Some(track.clone()));
        if let Some(route) = self.route.as_mut() {
            route.track = track;
        }
        self.prepared = None;
        self.prepare();
    }

    fn substitute(&mut self, unplayable: &Track) {
        let fallbacks = self.fallback_catalogs(unplayable);
        if fallbacks.is_empty() {
            self.cannot_play(unplayable);
            return;
        }
        let alternatives = self.known.alternatives(unplayable);
        let known = fallbacks.iter().find_map(|fallback| {
            alternatives
                .iter()
                .find(|(service, _)| *service == fallback.service.slug)
                .map(|(_, id)| (fallback.key.clone(), fallback.service, id.clone()))
        });
        if let Some((key, service, id)) = known {
            let track = Track {
                service,
                id,
                ..unplayable.clone()
            };
            self.sound(key, track);
            return;
        }
        if let Some(previous) = self.playback() {
            previous.pause();
        }
        self.route = None;
        let looked_for = unplayable.clone();
        let seek_finished = unplayable.clone();
        spawn_keyed(
            SUBSTITUTE,
            async move {
                let results =
                    join_all(fallbacks.iter().map(|fallback| {
                        music_blend::find(fallback.catalog.as_ref(), &seek_finished)
                    }))
                    .await;
                fallbacks
                    .into_iter()
                    .zip(results)
                    .find_map(|(fallback, result)| Some((fallback.key, result.ok()??)))
            },
            move |player: &mut Player, substitute: Option<(String, Track)>| {
                player.substituted(&looked_for, substitute);
            },
        );
    }

    pub(crate) fn substituted(&mut self, unplayable: &Track, substitute: Option<(String, Track)>) {
        if !self.is_now(unplayable) {
            return;
        }
        match substitute {
            Some((key, track)) => self.sound(key, track),
            None => self.cannot_play(unplayable),
        }
    }

    fn cannot_play(&mut self, unplayable: &Track) {
        self.error_message.set(Some(
            tr!("music.player.nowhere", name = unplayable.service.name).to_string(),
        ));
        self.miss();
    }

    fn miss(&mut self) {
        self.misses += 1;
        match self.misses >= self.queue.len() {
            true => self.stop(),
            false => self.next(),
        }
    }

    pub(crate) fn serving(&self, service: &str) -> Option<&Connection> {
        self.linked_for(service).map(|linked| &linked.connection)
    }

    fn native(&self, track: &Track) -> Option<String> {
        self.linked_for(track.service.slug)
            .map(|linked| linked.key.clone())
    }

    fn linked_for(&self, service: &str) -> Option<&LinkedAccount> {
        let serving = |linked: &&LinkedAccount| linked.connection.service.slug == service;
        self.links
            .iter()
            .filter(serving)
            .find(|linked| self.browsed() == Some(linked.key.as_str()))
            .or_else(|| self.links.iter().find(serving))
    }

    fn fallback_catalogs(&self, unplayable: &Track) -> Vec<FallbackCatalog> {
        let rank = |slug: &str| {
            self.order
                .iter()
                .position(|preferred| preferred == slug)
                .unwrap_or(self.order.len())
        };
        let mut fallbacks: Vec<&LinkedAccount> = self
            .links
            .iter()
            .filter(|linked| linked.connection.service.slug != unplayable.service.slug)
            .collect();
        fallbacks.sort_by_key(|linked| {
            (
                rank(linked.connection.service.slug),
                self.browsed() != Some(linked.key.as_str()),
            )
        });
        fallbacks.dedup_by_key(|linked| linked.connection.service.slug);
        fallbacks
            .into_iter()
            .map(|linked| FallbackCatalog {
                key: linked.key.clone(),
                service: linked.connection.service,
                catalog: Arc::clone(&linked.connection.catalog),
            })
            .collect()
    }

    fn finish(&mut self) {
        if let Some(playback) = self.playback() {
            playback.pause();
        }
        let length = self.clock.length();
        self.clock.stop(length);
        self.playing.set(false);
        self.announce();
    }

    fn stop(&mut self) {
        cancel(SUBSTITUTE);
        self.finish();
        self.loading.set(false);
    }

    fn handle_event_from(&mut self, key: &str, event: PlaybackEvent) {
        match self.route.as_ref().is_some_and(|route| route.key == key) {
            true => self.handle_event(event),
            false => skip_view(),
        }
    }

    pub(crate) fn handle_event(&mut self, event: PlaybackEvent) {
        self.apply_event(event);
        self.announce();
    }

    fn apply_event(&mut self, event: PlaybackEvent) {
        let now = Instant::now();
        let Some(route) = self.route.clone() else {
            return;
        };
        if let Some(track) = event.track()
            && *track != route.track.id
        {
            return;
        }
        let substituted = self.via.is_some();
        match event {
            PlaybackEvent::Loading { .. } => self.loading.set(true),
            PlaybackEvent::Playing { at, .. } => {
                self.misses = 0;
                self.loading.set(false);
                self.playing.set(true);
                self.clock.run(at, now);
            }
            PlaybackEvent::Paused { at, .. } => {
                self.loading.set(false);
                self.playing.set(false);
                self.clock.stop(at);
            }
            PlaybackEvent::Position { at, .. } => {
                let shown = self.clock.position(now);
                if shown.abs_diff(at) > DRIFT {
                    match *self.playing {
                        true => self.clock.run(at, now),
                        false => self.clock.stop(at),
                    }
                }
            }
            PlaybackEvent::Length { length, .. } => {
                if self.clock.length() != length {
                    self.clock.set_length(length);
                }
            }
            PlaybackEvent::Sounding { .. } => {}
            PlaybackEvent::Ended { .. } => match self.queue.advance(true).is_some() {
                true => self.begin(),
                false => self.finish(),
            },
            PlaybackEvent::Advanced { to, .. } => self.advanced(&to, now),
            PlaybackEvent::Unavailable { .. } | PlaybackEvent::Refused { .. }
                if !substituted && !self.fallback_catalogs(&route.track).is_empty() =>
            {
                self.substitute(&route.track);
            }
            PlaybackEvent::Unavailable { .. } => {
                self.error_message
                    .set(Some(tr!("music.player.unavailable").to_string()));
                self.miss();
            }
            PlaybackEvent::Refused { .. } => {
                self.stop();
                let service = route.track.service.name;
                self.error_message.set(Some(
                    tr!("music.player.refused", name = service).to_string(),
                ));
            }
            PlaybackEvent::Failed(reason) => self.error_message.set(Some(reason)),
        }
    }

    fn playback(&self) -> Option<Arc<dyn Playback>> {
        self.route
            .as_ref()
            .and_then(|route| self.playback_of(&route.key))
    }

    fn playback_of(&self, key: &str) -> Option<Arc<dyn Playback>> {
        self.links
            .iter()
            .find(|linked| linked.key == key)
            .map(|linked| Arc::clone(&linked.connection.playback))
    }
}

fn events_key(key: &str) -> (&'static str, String) {
    ("music-events", key.to_owned())
}

fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos() as u64)
}

impl std::fmt::Debug for Player {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Player")
            .field("now", &self.now.as_ref().map(|track| &track.id))
            .field("playing", &*self.playing)
            .finish_non_exhaustive()
    }
}

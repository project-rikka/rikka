use std::time::Duration;

use rikka_app::prelude::*;

use crate::screen::Screen;
use crate::transport::{self, Playback};

const TICKING: &str = "viewer.ticking";

#[derive(Debug)]
pub(crate) struct Ticker {
    period: Duration,
    running: bool,
}

impl Ticker {
    pub(crate) const fn new(period: Duration) -> Self {
        Self {
            period,
            running: false,
        }
    }

    pub(crate) fn start(&mut self) {
        if std::mem::replace(&mut self.running, true) {
            return;
        }
        every_keyed(TICKING, self.period, |screen: &mut Screen| screen.on_tick());
    }

    pub(crate) fn stop(&mut self) {
        self.running = false;
        cancel(TICKING);
    }

    pub(crate) fn stop_all() {
        cancel(TICKING);
    }
}

pub(crate) trait Player {
    fn volume(&self) -> Option<f32>;
    fn set_volume(&self, volume: f32);
    fn is_playing(&self) -> bool;
    fn position(&self) -> Duration;
    fn duration(&self) -> Option<Duration>;
    fn is_dead(&self) -> bool;

    fn start(&mut self);
    fn play_pause(&mut self);
    fn seek(&mut self, position: Duration);

    fn on_tick(&mut self) {}

    fn detail(&self) -> Option<Element> {
        None
    }

    fn skip(&mut self, seconds: f32) {
        let position = Duration::from_secs_f32((self.position().as_secs_f32() + seconds).max(0.0));
        self.seek(match self.duration() {
            Some(duration) => position.min(duration),
            None => position,
        });
    }

    fn nudge_volume(&self, step: f32) {
        if let Some(volume) = self.volume() {
            self.set_volume(volume + step);
        }
    }

    fn controls(&self) -> Element {
        transport::transport(Playback {
            playing: self.is_playing(),
            position: self.position(),
            duration: self.duration(),
            volume: self.volume(),
            dead: self.is_dead(),
        })
        .build()
    }
}

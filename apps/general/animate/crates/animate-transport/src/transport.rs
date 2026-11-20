use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use animate_cast::{Clock, SharedClock, frame_of, time_of, with_clock};
use rikka_audio_decode::Sound;
use rikka_ui::Observers;

use crate::soundtrack::{Peaks, Soundtrack};

const DRIFT: f32 = 0.05;
const SYNC_GRACE: Duration = Duration::from_millis(250);
const LAP: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Running,
    Ended,
}

#[derive(Clone, Debug, Default)]
pub struct Transport(Rc<Inner>);

#[derive(Debug)]
struct Inner {
    clock: SharedClock,
    sound: RefCell<Option<Soundtrack>>,
    sync_after: Cell<Instant>,
    last: Cell<f32>,
    position: Observers,
    state: Observers,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            clock: SharedClock::default(),
            sound: RefCell::new(None),
            sync_after: Cell::new(Instant::now()),
            last: Cell::new(0.0),
            position: Observers::new(),
            state: Observers::new(),
        }
    }
}

impl Transport {
    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.0.clock)
    }

    fn with_clock<R>(&self, use_clock: impl FnOnce(&mut Clock) -> R) -> R {
        with_clock(&self.0.clock, use_clock)
    }

    pub fn time(&self) -> f32 {
        self.0.position.read();
        self.with_clock(|clock| clock.time(Instant::now()))
    }

    pub fn frame(&self) -> u32 {
        frame_of(self.time())
    }

    pub fn last_frame(&self) -> u32 {
        frame_of(self.length())
    }

    pub fn is_playing(&self) -> bool {
        self.0.state.read();
        self.with_clock(|clock| clock.is_running())
    }

    pub fn length(&self) -> f32 {
        self.0.state.read();
        self.with_clock(|clock| clock.length())
    }

    pub fn repeats(&self) -> bool {
        self.0.state.read();
        self.with_clock(|clock| clock.repeats())
    }

    pub fn peaks(&self) -> Option<Arc<Peaks>> {
        self.0.state.read();
        self.0
            .sound
            .borrow()
            .as_ref()
            .map(|sound| Arc::clone(sound.peaks()))
    }

    pub fn sound_name(&self) -> Option<String> {
        self.0.state.read();
        self.0
            .sound
            .borrow()
            .as_ref()
            .map(|sound| sound.name().to_owned())
    }

    pub fn sound(&self) -> Option<Sound> {
        self.0.state.read();
        self.0.sound.borrow().as_ref().and_then(Soundtrack::sound)
    }

    pub fn sound_length(&self) -> Option<f32> {
        self.0.sound.borrow().as_ref().map(Soundtrack::length)
    }

    pub fn volume(&self) -> Option<f32> {
        self.0.state.read();
        self.0.sound.borrow().as_ref().map(Soundtrack::volume)
    }

    pub fn set_volume(&self, volume: f32) {
        if let Some(sound) = self.0.sound.borrow().as_ref() {
            sound.set_volume(volume);
        }
    }

    pub fn play(&self) {
        let now = Instant::now();
        let started = self.with_clock(|clock| {
            clock.run(now);
            clock.is_running().then(|| clock.time(now))
        });
        let Some(time) = started else {
            return;
        };
        if let Some(sound) = self.0.sound.borrow_mut().as_mut() {
            sound.play_from(time);
        }
        self.mark_seek(time);
        self.0.state.changed();
    }

    pub fn pause(&self) {
        let now = Instant::now();
        let time = self.with_clock(|clock| {
            clock.stop(now);
            clock.time(now)
        });
        if let Some(sound) = self.0.sound.borrow().as_ref() {
            sound.pause();
        }
        self.mark_seek(time);
        self.0.state.changed();
    }

    pub fn toggle(&self) {
        match self.with_clock(|clock| clock.is_running()) {
            true => self.pause(),
            false => self.play(),
        }
    }

    pub fn seek(&self, time: f32) {
        let now = Instant::now();
        let (time, running) = self.with_clock(|clock| {
            clock.seek(time, now);
            (clock.time(now), clock.is_running())
        });
        if let Some(sound) = self.0.sound.borrow_mut().as_mut() {
            sound.seek(time, running);
        }
        self.mark_seek(time);
    }

    pub fn seek_frame(&self, frame: u32) {
        self.seek(time_of(frame));
    }

    pub fn step(&self, frames: i64) {
        if self.with_clock(|clock| clock.is_running()) {
            self.pause();
        }
        let frame = (i64::from(self.frame()) + frames).clamp(0, i64::from(u32::MAX));
        self.seek_frame(frame as u32);
    }

    pub fn set_length(&self, length: f32) {
        let now = Instant::now();
        let changed = self.with_clock(|clock| {
            if clock.length() == length.max(0.0) {
                return false;
            }
            clock.set_length(length, now);
            if clock.has_ended(now) {
                clock.stop(now);
            }
            true
        });
        if changed {
            self.0.state.changed();
            self.0.position.changed();
        }
    }

    pub fn set_repeat(&self, repeat: bool) {
        let changed = self.with_clock(|clock| {
            if clock.repeats() == repeat {
                return false;
            }
            clock.set_repeat(repeat, Instant::now());
            true
        });
        if changed {
            self.0.state.changed();
        }
    }

    pub fn set_soundtrack(&self, sound: Option<Soundtrack>) {
        let now = Instant::now();
        let (time, running) = self.with_clock(|clock| (clock.time(now), clock.is_running()));
        let mut slot = self.0.sound.borrow_mut();
        *slot = sound;
        if let Some(sound) = slot.as_mut() {
            sound.seek(time, running);
        }
        drop(slot);
        self.mark_seek(time);
        self.0.state.changed();
    }

    pub fn sync_to_sound(&self) -> PlaybackState {
        let now = Instant::now();
        let state = self.with_clock(|clock| {
            if !clock.is_running() {
                return ClockState::Stopped;
            }
            if clock.has_ended(now) {
                clock.stop(now);
                return ClockState::Ended;
            }
            ClockState::Running(clock.time(now))
        });
        let time = match state {
            ClockState::Stopped => return PlaybackState::Ended,
            ClockState::Ended => {
                if let Some(sound) = self.0.sound.borrow().as_ref() {
                    sound.pause();
                }
                self.0.state.changed();
                self.0.position.changed();
                return PlaybackState::Ended;
            }
            ClockState::Running(time) => time,
        };
        let lapped = time + LAP < self.0.last.replace(time);
        let mut sound = self.0.sound.borrow_mut();
        let Some(sound) = sound.as_mut() else {
            return PlaybackState::Running;
        };
        if lapped {
            sound.play_from(time);
            self.0.sync_after.set(now + SYNC_GRACE);
            return PlaybackState::Running;
        }
        if now < self.0.sync_after.get() {
            return PlaybackState::Running;
        }
        if let Some(audible_time) = sound.audible_time()
            && (audible_time - time).abs() > DRIFT
        {
            self.with_clock(|clock| clock.seek(audible_time, now));
        }
        PlaybackState::Running
    }

    fn mark_seek(&self, time: f32) {
        self.0.last.set(time);
        self.0.sync_after.set(Instant::now() + SYNC_GRACE);
        self.0.position.changed();
    }
}

enum ClockState {
    Stopped,
    Ended,
    Running(f32),
}

impl PartialEq for Transport {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

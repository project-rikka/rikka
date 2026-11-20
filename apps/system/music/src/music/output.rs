use rikka_app::prelude::*;
use rikka_hifi::{Output, ReplayGain};
use rikka_sound::config::GraphTuning;

use crate::library::Library;
use crate::music::Music;
use crate::sound::Sound;

impl Music {
    pub(crate) fn sound(&self) -> &Sound {
        &self.sound
    }

    pub(crate) fn signal_open(&self) -> bool {
        *self.signal_open
    }

    pub(crate) fn open_signal(&mut self) {
        self.menus.close();
        self.sound.watch_graph();
        self.sound.list_devices();
        self.signal_open.set(true);
    }

    pub(crate) fn close_signal(&mut self) {
        if *self.signal_open {
            self.signal_open.set(false);
        }
    }

    pub(crate) fn set_output(&mut self, output: Output) {
        if self.sound.output() == &output {
            return;
        }
        self.preferences.set_output(&output);
        self.sound.choose(output);
    }

    pub(crate) fn set_replay_gain(&mut self, replay_gain: ReplayGain) {
        if self.sound.replay_gain() == replay_gain {
            return;
        }
        self.preferences.set_replay_gain(replay_gain);
        self.sound.level(replay_gain);
    }

    pub(crate) fn allow_rate_switching(&mut self, rates: Vec<u32>) {
        self.sound.allow_rates(&rates);
        spawn_blocking(
            move || {
                let mut tuning = GraphTuning::read();
                tuning.allowed_rates = rates;
                tuning.write().map_err(|error| error.to_string())
            },
            |library: &mut Library, saved: Result<(), String>| match saved {
                Ok(()) => library.show_note(tr!("music.signal.switching").to_string()),
                Err(reason) => library.fail(&reason),
            },
        );
    }
}

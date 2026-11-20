use discord_voice::{
    InputMode, KrispState, MicTest, NoiseSuppression, VoiceSettings, input_devices, output_devices,
};
use rikka_app::prelude::*;

use super::Chat;
use crate::preferences::{Alerts, Appearance};
use crate::settings::{Listed, Section, Settings};

const DEVICES: &str = "discord-voice-devices";
const MIC_TEST: &str = "discord-mic-test";
const LEVELS: &str = "discord-mic-levels";
const KRISP: &str = "discord-krisp";

impl Chat {
    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn open_settings(&mut self, section: Section) {
        self.close_overlays();
        self.settings.open(section);
        self.entered_section(section);
    }

    pub(crate) fn close_settings(&mut self) {
        cancel(MIC_TEST);
        cancel(LEVELS);
        retire(self.settings.close());
    }

    pub(crate) fn show_settings(&mut self, section: Section) {
        cancel(MIC_TEST);
        cancel(LEVELS);
        retire(self.settings.show(section));
        self.entered_section(section);
    }

    fn entered_section(&mut self, section: Section) {
        if section != Section::Voice {
            return;
        }
        spawn_blocking_keyed(
            DEVICES,
            || {
                let inputs = input_devices().map_err(|error| error.to_string())?;
                let outputs = output_devices().map_err(|error| error.to_string())?;
                Ok(Listed { inputs, outputs })
            },
            |settings: &mut Settings, listed: Result<Listed, String>| settings.listed(listed),
        );
    }

    pub(crate) fn voice_settings(&self) -> VoiceSettings {
        self.preferences.voice()
    }

    pub(crate) fn change_voice(&mut self, change: impl FnOnce(&mut VoiceSettings)) {
        let mut settings = self.preferences.voice();
        let before = settings.noise_suppression;
        change(&mut settings);
        if settings.noise_suppression == NoiseSuppression::Krisp
            && before != NoiseSuppression::Krisp
        {
            self.prepare_krisp();
        }
        let testing = self.settings.testing();
        self.set_voice_settings(settings);
        if testing {
            self.start_mic_test();
        }
    }

    pub(crate) fn krisp_state(&self) -> &KrispState {
        &self.krisp_state
    }

    pub(crate) fn retry_krisp(&mut self) {
        self.prepare_krisp();
    }

    fn prepare_krisp(&mut self) {
        if let Err(error) = self.krisp.prepare() {
            tracing::warn!(%error, "failed to prepare Krisp");
            self.krisp_state
                .set(KrispState::Unavailable(error.to_string()));
        }
    }

    pub(crate) fn watch_krisp(&mut self) {
        if self.preferences.voice().noise_suppression == NoiseSuppression::Krisp {
            self.prepare_krisp();
        }
        stream_keyed(
            KRISP,
            self.krisp.watch(),
            |chat: &mut Chat, state: KrispState| {
                chat.krisp_state.set(state);
            },
        );
    }

    pub(crate) fn set_talking_mode(&mut self, push_to_talk: bool) {
        self.change_voice(|settings| {
            settings.mode = match push_to_talk {
                true => InputMode::PushToTalk {
                    release: std::time::Duration::from_millis(200),
                },
                false => InputMode::default(),
            };
        });
    }

    pub(crate) fn talk_key(&self) -> Key {
        self.preferences.talk_key()
    }

    pub(crate) fn set_talk_key(&mut self, key: Key) {
        self.settings.record(false);
        self.preferences.set_talk_key(key);
    }

    pub(crate) fn start_mic_test(&mut self) {
        cancel(LEVELS);
        retire(self.settings.stop());
        let settings = self.preferences.voice();
        let krisp = self.krisp.clone();
        spawn_blocking_keyed(
            MIC_TEST,
            move || MicTest::start(&settings, &krisp).map_err(|error| error.to_string()),
            |settings: &mut Settings, started: Result<MicTest, String>| match started {
                Ok(mut test) => {
                    let levels = test.levels();
                    settings.tested(test);
                    stream_keyed(LEVELS, levels, |settings: &mut Settings, level: f32| {
                        settings.set_mic_level(level);
                    });
                }
                Err(error_message) => settings.failed(error_message),
            },
        );
    }

    pub(crate) fn stop_mic_test(&mut self) {
        cancel(MIC_TEST);
        cancel(LEVELS);
        retire(self.settings.stop());
    }

    pub(crate) fn alerts(&self) -> Alerts {
        self.preferences.alerts()
    }

    pub(crate) fn set_alerts(&mut self, alerts: Alerts) {
        self.preferences.set_alerts(alerts);
    }

    pub(crate) fn appearance(&self) -> Appearance {
        *self.appearance
    }

    pub(crate) fn set_appearance(&mut self, appearance: Appearance) {
        self.preferences.set_appearance(appearance);
        self.appearance.set(appearance);
    }
}

fn retire(test: Option<MicTest>) {
    if let Some(test) = test {
        spawn_blocking(move || drop(test), |(): ()| skip_view());
    }
}

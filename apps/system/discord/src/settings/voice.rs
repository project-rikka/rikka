use discord_voice::{Device, InputMode, KrispState, MAX_VOLUME, NoiseSuppression, VoiceSettings};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::Settings;
use crate::chat::Chat;
use crate::voice::hotkey;

const CHOICE: f32 = 260.0;
const SLIDER: f32 = 220.0;
const FLOOR_DB: f32 = -100.0;

pub(super) fn view(chat: &Chat) -> Element {
    let settings = chat.voice_settings();
    let shown = chat.settings();
    let devices = match shown.devices() {
        Some(Ok(listed)) => devices(&settings, &listed.inputs, &listed.outputs).any(),
        Some(Err(error)) => text(error.clone()).caption().danger().any(),
        None => spinner().any(),
    };
    column! {
        section(tr!("discord.settings.devices"), devices);
        section(tr!("discord.settings.volume"), volumes(&settings));
        section(tr!("discord.settings.input-mode"), mode(chat, &settings));
        section(tr!("discord.settings.processing"), processing(chat, &settings));
        section(tr!("discord.settings.mic-test"), mic_test(shown));
    }
    .gap(Space::LOOSE)
    .fill_width()
    .any()
}

fn suppression_label(kind: NoiseSuppression) -> Label {
    match kind {
        NoiseSuppression::Off => tr!("discord.settings.suppression-off"),
        NoiseSuppression::Standard => tr!("discord.settings.suppression-standard"),
        NoiseSuppression::Krisp => tr!("discord.settings.suppression-krisp"),
    }
}

fn processing(chat: &Chat, settings: &VoiceSettings) -> impl View {
    let chosen = NoiseSuppression::ALL
        .iter()
        .position(|kind| *kind == settings.noise_suppression)
        .unwrap_or(0);
    let status = (settings.noise_suppression == NoiseSuppression::Krisp)
        .then(|| krisp_status(chat.krisp_state()));
    column! {
        field(
            tr!("discord.settings.noise-suppression"),
            NoiseSuppression::ALL
                .into_iter()
                .fold(segmented(chosen), |choice, kind| choice.segment(suppression_label(kind)))
                .on_select(|chat: &mut Chat, at: usize| {
                    if let Some(kind) = NoiseSuppression::ALL.get(at).copied() {
                        chat.change_voice(|settings| settings.noise_suppression = kind);
                    }
                })
                .width(CHOICE),
        );
        if let Some(status) = status {
            status;
        }
        field(
            tr!("discord.settings.echo-cancellation"),
            switch(settings.echo_cancellation).on_toggle(|chat: &mut Chat, on: bool| {
                chat.change_voice(|settings| settings.echo_cancellation = on);
            }),
        );
        field(
            tr!("discord.settings.automatic-gain"),
            switch(settings.automatic_gain).on_toggle(|chat: &mut Chat, on: bool| {
                chat.change_voice(|settings| settings.automatic_gain = on);
            }),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn krisp_status(state: &KrispState) -> Element {
    match state {
        KrispState::Idle | KrispState::Preparing => text(tr!("discord.settings.krisp-preparing"))
            .caption()
            .muted()
            .any(),
        KrispState::Downloading { .. } => text(tr!(
            "discord.settings.krisp-downloading",
            percent = i64::from(state.percent().unwrap_or(0))
        ))
        .caption()
        .muted()
        .any(),
        KrispState::Ready => text(tr!("discord.settings.krisp-ready"))
            .caption()
            .muted()
            .any(),
        KrispState::Unavailable(reason) => row! {
            text(tr!("discord.settings.krisp-unavailable", reason = reason.as_str()))
                .caption()
                .danger()
                .grow(1.0);
            button(text(tr!("discord.settings.krisp-retry")))
                .secondary()
                .on_press(|chat: &mut Chat| chat.retry_krisp());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .any(),
    }
}

fn devices(settings: &VoiceSettings, inputs: &[Device], outputs: &[Device]) -> impl View {
    column! {
        field(
            tr!("discord.settings.input-device"),
            device_choice(settings.input.as_deref(), inputs, |settings, name| settings.input = name),
        );
        field(
            tr!("discord.settings.output-device"),
            device_choice(settings.output.as_deref(), outputs, |settings, name| settings.output = name),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn device_choice(
    chosen: Option<&str>,
    devices: &[Device],
    choose: fn(&mut VoiceSettings, Option<String>),
) -> impl View {
    let names: Vec<String> = devices.iter().map(|device| device.name.clone()).collect();
    let at = match chosen {
        None => 0,
        Some(chosen) => names
            .iter()
            .position(|name| name == chosen)
            .map_or(0, |at| at + 1),
    };
    let mut options: Vec<Label> = vec![tr!("discord.settings.default-device")];
    options.extend(names.iter().cloned().map(Label::from));
    combo(at)
        .options(options)
        .on_select(move |chat: &mut Chat, index: usize| {
            let name = index.checked_sub(1).and_then(|at| names.get(at)).cloned();
            chat.change_voice(|settings| choose(settings, name));
        })
        .width(CHOICE)
}

fn volumes(settings: &VoiceSettings) -> impl View {
    column! {
        field(
            tr!("discord.settings.input-volume"),
            volume(settings.input_volume, |settings, volume| settings.input_volume = volume),
        );
        field(
            tr!("discord.settings.output-volume"),
            volume(settings.output_volume, |settings, volume| settings.output_volume = volume),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn volume(value: f32, set: fn(&mut VoiceSettings, f32)) -> impl View {
    row! {
        slider(value)
            .range(0.0..=MAX_VOLUME)
            .step(0.05)
            .on_commit(move |chat: &mut Chat, volume: f32| {
                chat.change_voice(|settings| set(settings, volume));
            })
            .width(SLIDER);
        text(format!("{}%", (value * 100.0).round() as i64)).caption().muted().no_wrap();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn mode(chat: &Chat, settings: &VoiceSettings) -> impl View {
    let (chosen, details) = match settings.mode {
        InputMode::VoiceActivity {
            threshold_db,
            automatic,
        } => (0, activity(threshold_db, automatic).any()),
        InputMode::PushToTalk { release } => (1, talking(chat, release).any()),
    };
    column! {
        segmented(chosen)
            .segment(tr!("discord.settings.voice-activity"))
            .segment(tr!("discord.settings.push-to-talk"))
            .on_select(|chat: &mut Chat, at: usize| chat.set_talking_mode(at == 1))
            .width(CHOICE);
        details;
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn activity(threshold_db: f32, automatic: bool) -> impl View {
    column! {
        field(
            tr!("discord.settings.automatic-sensitivity"),
            switch(automatic).on_toggle(move |chat: &mut Chat, on: bool| {
                chat.change_voice(|settings| {
                    settings.mode = InputMode::VoiceActivity {
                        threshold_db,
                        automatic: on,
                    };
                });
            }),
        );
        field(
            tr!("discord.settings.sensitivity"),
            row! {
                slider(threshold_db)
                    .range(FLOOR_DB..=0.0)
                    .step(1.0)
                    .enabled(!automatic)
                    .on_commit(move |chat: &mut Chat, threshold_db: f32| {
                        chat.change_voice(|settings| {
                            settings.mode = InputMode::VoiceActivity {
                                threshold_db,
                                automatic,
                            };
                        });
                    })
                    .width(SLIDER);
                text(tr!("discord.settings.decibels", db = threshold_db.round() as i64))
                    .caption()
                    .muted()
                    .no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn talking(chat: &Chat, release: std::time::Duration) -> impl View {
    let recording = chat.settings().recording();
    column! {
        field(
            tr!("discord.settings.talk-key"),
            shortcut_field(hotkey::label(chat.talk_key()))
                .prompt(tr!("discord.settings.press-key"))
                .recording(recording)
                .on_record(|settings: &mut Settings| settings.record(true))
                .on_cancel(|settings: &mut Settings| settings.record(false))
                .on_capture(|chat: &mut Chat, _: Modifiers, key: Key| chat.set_talk_key(key))
                .width(CHOICE),
        );
        field(
            tr!("discord.settings.talk-release"),
            row! {
                slider(release.as_millis() as f32)
                    .range(0.0..=2000.0)
                    .step(20.0)
                    .on_commit(|chat: &mut Chat, millis: f32| {
                        chat.change_voice(|settings| {
                            settings.mode = InputMode::PushToTalk {
                                release: std::time::Duration::from_millis(millis as u64),
                            };
                        });
                    })
                    .width(SLIDER);
                text(tr!("discord.settings.milliseconds", ms = release.as_millis() as i64))
                    .caption()
                    .muted()
                    .no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn mic_test(settings: &Settings) -> impl View {
    let testing = settings.testing();
    let level = settings
        .level()
        .filter(|level| level.is_finite())
        .map_or(0.0, |level| {
            ((level - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
        });
    column! {
        text(tr!("discord.settings.mic-test-hint")).caption().muted().fill_width();
        row! {
            match testing {
                true => button(text(tr!("discord.settings.stop-test")))
                    .secondary()
                    .on_press(|chat: &mut Chat| chat.stop_mic_test())
                    .any(),
                false => button(text(tr!("discord.settings.start-test")))
                    .primary()
                    .on_press(|chat: &mut Chat| chat.start_mic_test())
                    .any(),
            };
            progress(level).color(palette().success).grow(1.0);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width();
        field(
            tr!("discord.settings.loopback"),
            switch(settings.looping()).on_toggle(|settings: &mut Settings, _: bool| settings.toggle_loopback()),
        );
        if let Some(error_message) = settings.error_message() {
            text(error_message.to_owned()).caption().danger().fill_width();
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

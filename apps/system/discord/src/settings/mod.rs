mod accounts;
mod alerts;
mod appearance;
mod section;
mod voice;

use discord_voice::{Device, MicTest};
use rikka_app::prelude::*;

use crate::chat::Chat;
pub(crate) use section::Section;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub(crate) inputs: Vec<Device>,
    pub(crate) outputs: Vec<Device>,
}

#[derive(State)]
pub(crate) struct Settings {
    open: Tracked<bool>,
    section: Tracked<Section>,
    devices: Tracked<Option<Result<Listed, String>>>,
    level: Tracked<Option<f32>>,
    looping: Tracked<bool>,
    recording: Tracked<bool>,
    error_message: Tracked<Option<String>>,
    #[untracked]
    test: Option<MicTest>,
}

impl Settings {
    pub(crate) fn new() -> Settings {
        Settings {
            open: Tracked::new(false),
            section: Tracked::new(Section::default()),
            devices: Tracked::new(None),
            level: Tracked::new(None),
            looping: Tracked::new(false),
            recording: Tracked::new(false),
            error_message: Tracked::new(None),
            test: None,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        *self.open
    }

    pub(crate) fn section(&self) -> Section {
        *self.section
    }

    pub(crate) fn devices(&self) -> Option<&Result<Listed, String>> {
        self.devices.as_ref()
    }

    pub(crate) fn level(&self) -> Option<f32> {
        *self.level
    }

    pub(crate) fn testing(&self) -> bool {
        self.level.is_some()
    }

    pub(crate) fn looping(&self) -> bool {
        *self.looping
    }

    pub(crate) fn recording(&self) -> bool {
        *self.recording
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn open(&mut self, section: Section) {
        self.open.set(true);
        self.section.set(section);
    }

    pub(crate) fn close(&mut self) -> Option<MicTest> {
        self.open.set(false);
        self.stop()
    }

    pub(crate) fn show(&mut self, section: Section) -> Option<MicTest> {
        self.section.set(section);
        self.error_message.set(None);
        self.recording.set(false);
        self.stop()
    }

    pub(crate) fn listed(&mut self, devices: Result<Listed, String>) {
        self.devices.set(Some(devices));
    }

    pub(crate) fn tested(&mut self, test: MicTest) {
        test.set_loopback(*self.looping);
        self.test = Some(test);
        self.level.set(Some(f32::NEG_INFINITY));
    }

    pub(crate) fn failed(&mut self, error_message: String) {
        self.level.set(None);
        self.error_message.set(Some(error_message));
    }

    pub(crate) fn stop(&mut self) -> Option<MicTest> {
        self.level.set(None);
        self.test.take()
    }

    pub(crate) fn set_mic_level(&mut self, level: f32) {
        if self.test.is_none() {
            skip_view();
            return;
        }
        self.level.set(Some(level));
    }

    pub(crate) fn toggle_loopback(&mut self) {
        let looping = !*self.looping;
        if let Some(test) = &self.test {
            test.set_loopback(looping);
        }
        self.looping.set(looping);
    }

    pub(crate) fn record(&mut self, recording: bool) {
        self.recording.set(recording);
    }
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("open", &*self.open)
            .field("section", &*self.section)
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(chat: &Chat) -> impl View {
    let section = chat.settings().section();
    split_view(nav(section), body(chat, section))
        .heading(section.title())
        .shows(section.index())
}

fn nav(section: Section) -> impl View {
    nav_list(section.index())
        .items(Section::ALL.map(Section::nav_item))
        .on_select(|chat: &mut Chat, index: usize| chat.show_settings(Section::from_index(index)))
}

fn body(chat: &Chat, section: Section) -> Element {
    match section {
        Section::Accounts => accounts::view(chat),
        Section::Voice => component(|chat: &Chat| chat, voice::view).any(),
        Section::Notifications => alerts::view(chat),
        Section::Appearance => appearance::view(chat),
    }
}

use rikka_app::prelude::*;

use crate::music::Music;
use crate::settings::section::Section;
use crate::settings::{accounts, library, output, quality, sources, storage};

#[derive(Debug, State)]
pub(crate) struct Settings {
    section: Tracked<Section>,
}

impl Settings {
    pub(crate) fn new() -> Settings {
        Settings {
            section: Tracked::new(Section::default()),
        }
    }

    pub(crate) fn show(&mut self, section: Section) {
        self.section.set(section);
    }

    fn section(&self) -> Section {
        *self.section
    }
}

pub(crate) fn view(music: &Music) -> impl View {
    let section = music.settings().section();
    split_view(nav(section), body(music, section))
        .heading(section.title())
        .shows(section.index())
}

fn nav(section: Section) -> impl View {
    nav_list(section.index())
        .items(Section::ALL.map(Section::nav_item))
        .on_select(|settings: &mut Settings, index: usize| {
            settings.show(Section::from_index(index))
        })
}

fn body(music: &Music, section: Section) -> Element {
    match section {
        Section::Accounts => accounts::view(music).any(),
        Section::Library => library::view(music),
        Section::Output => output::view(music).any(),
        Section::Quality => quality::view(music.preferences()).any(),
        Section::Sources => sources::view(music).any(),
        Section::Storage => storage::view(music).any(),
    }
}

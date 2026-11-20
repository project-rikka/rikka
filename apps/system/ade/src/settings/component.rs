use std::path::PathBuf;

use rikka_app::prelude::*;
use rikka_chat::{Servers, servers};
use rikka_llm_settings::{LlmSettings, llm_settings};
use rikka_mcp::Hub;

use crate::project;
use crate::settings::commands::{self, Commands};
use crate::settings::section::Section;
use crate::settings::skills::{self, Skills};

#[derive(Debug, Default, State)]
pub(crate) struct Settings {
    section: Section,
    #[state]
    llm: Option<LlmSettings>,
    #[state]
    servers: Option<Servers>,
    skills: Skills,
    commands: Commands,
}

impl Settings {
    pub(crate) fn reread(&mut self, root: Option<PathBuf>, hub: Hub) {
        self.llm.get_or_insert_with(LlmSettings::load).open();
        let mut servers = Servers::new(hub);
        servers.follow();
        self.servers = Some(servers);
        self.skills.reread(root.clone());
        self.commands.reread(root);
    }

    pub(crate) fn select(&mut self, section: Section) {
        self.section = section;
    }

    pub(crate) fn leave(&mut self) {
        if let Some(llm) = self.llm.as_mut() {
            llm.close();
        }
        self.servers = None;
    }

    fn at(&self) -> usize {
        Section::ALL
            .iter()
            .position(|section| *section == self.section)
            .unwrap_or_default()
    }
}

pub(crate) fn view(settings: &Settings) -> impl View {
    split_view(nav(settings), body(settings))
        .heading(settings.section.title())
        .shows(settings.at())
}

fn nav(settings: &Settings) -> impl View {
    nav_list(settings.at())
        .items(Section::ALL.map(Section::nav_item))
        .on_select(|settings: &mut Settings, index: usize| {
            settings.section = Section::ALL.get(index).copied().unwrap_or_default();
        })
}

fn body(settings: &Settings) -> Element {
    match settings.section {
        Section::Llm => match &settings.llm {
            Some(llm) => llm_settings(llm).any(),
            None => spacer().any(),
        },
        Section::Servers => match &settings.servers {
            Some(shown) => servers(shown, project::titled).any(),
            None => spacer().any(),
        },
        Section::Skills => skills::view(&settings.skills).any(),
        Section::Commands => commands::view(&settings.commands).any(),
    }
}

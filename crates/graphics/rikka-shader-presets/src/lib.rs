mod classic;
mod games;
pub mod import;
mod styles;

use rikka_shader_graph::{Error, Graph, Library};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Family {
    Classic,
    Game,
    Stylized,
    Realistic,
}

impl Family {
    pub const ALL: [Self; 4] = [Self::Classic, Self::Game, Self::Stylized, Self::Realistic];

    pub fn name(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Game => "game",
            Self::Stylized => "stylized",
            Self::Realistic => "realistic",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub key: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub family: Family,
    pub build: fn(&Library) -> Result<Graph, Error>,
}

impl Preset {
    pub fn graph(&self, library: &Library) -> Result<Graph, Error> {
        (self.build)(library)
    }
}

const SETS: &[fn() -> Vec<Preset>] = &[classic::presets, games::presets, styles::presets];

pub fn presets() -> Vec<Preset> {
    SETS.iter().flat_map(|set| set()).collect()
}

pub fn preset(key: &str) -> Option<Preset> {
    presets().into_iter().find(|preset| preset.key == key)
}

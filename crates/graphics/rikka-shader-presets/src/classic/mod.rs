mod character;

use rikka_shader_graph::{Error, Graph, Library};

use crate::{Family, Preset};

use self::character::{Style, character};

fn mmd(library: &Library) -> Result<Graph, Error> {
    character(library, Style::MMD)
}

fn mtoon(library: &Library) -> Result<Graph, Error> {
    character(library, Style::MTOON)
}

fn anime(library: &Library) -> Result<Graph, Error> {
    character(library, Style::ANIME)
}

pub(crate) fn presets() -> Vec<Preset> {
    vec![
        Preset {
            key: "mmd",
            title: "MMD",
            summary: "MikuMikuDance 9.32 shading: material colour with ambient, toon ramp, sphere map, Blinn-Phong specular, self shadow and screen-width edges.",
            family: Family::Classic,
            build: mmd,
        },
        Preset {
            key: "mtoon",
            title: "MToon",
            summary: "VRM MToon 1.0: shade colour and texture, shading shift and toony, GI equalization, parametric and matcap rim, emission, outlines in world or screen width, uv animation.",
            family: Family::Classic,
            build: mtoon,
        },
        Preset {
            key: "anime",
            title: "Anime",
            summary: "MMD and MToon in one graph with anime figure touches that start at zero: hair shadow on the face, procedural face shadow, skin scatter bands, strand highlights and a depth rim.",
            family: Family::Stylized,
            build: anime,
        },
    ]
}

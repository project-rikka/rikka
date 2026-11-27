use std::path::Path;

use animate_document::Motion;
use rikka_media_format::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Pmx,
    Vrm,
    Fbx,
    Gltf,
    Glb,
    Blend,
    Stl,
    Vmd,
    Vrma,
    Motion,
    Sound(&'static str),
}

impl Format {
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Format> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        let format = match extension.as_str() {
            "pmx" => Format::Pmx,
            "vrm" => Format::Vrm,
            "fbx" => Format::Fbx,
            "gltf" => Format::Gltf,
            "glb" => Format::Glb,
            "blend" => Format::Blend,
            "stl" => Format::Stl,
            "vmd" => Format::Vmd,
            "vrma" => Format::Vrma,
            extension if extension == Motion::EXTENSION => Format::Motion,
            _ => {
                let sound = rikka_media_format::Format::from_path(path)
                    .filter(|format| format.kind() == Kind::Sound)?;
                Format::Sound(sound.name())
            }
        };
        Some(format)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Format::Pmx => "PMX",
            Format::Vrm => "VRM",
            Format::Fbx => "FBX",
            Format::Gltf => "glTF",
            Format::Glb => "GLB",
            Format::Blend => "Blend",
            Format::Stl => "STL",
            Format::Vmd => "VMD",
            Format::Vrma => "VRMA",
            Format::Motion => "Rikka Motion",
            Format::Sound(name) => name,
        }
    }

    #[must_use]
    pub const fn is_model(self) -> bool {
        matches!(
            self,
            Format::Pmx
                | Format::Vrm
                | Format::Fbx
                | Format::Gltf
                | Format::Glb
                | Format::Blend
                | Format::Stl
        )
    }

    pub(crate) const fn cost(self) -> Cost {
        match self {
            Format::Pmx | Format::Vmd | Format::Stl | Format::Blend => Cost::Header,
            Format::Vrm
            | Format::Vrma
            | Format::Gltf
            | Format::Glb
            | Format::Motion
            | Format::Sound(_) => Cost::Document,
            Format::Fbx => Cost::Import,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Cost {
    Header,
    Document,
    Import,
}

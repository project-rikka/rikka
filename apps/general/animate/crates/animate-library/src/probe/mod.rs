mod blend;
mod document;
mod fault;
mod fbx;
mod gltf;
mod pmx;
mod sound;
mod stl;
mod vmd;

use std::path::Path;
use std::sync::Arc;

use rikka_types::CancelToken;

use crate::details::Details;
use crate::format::Format;

pub(crate) use gltf::embedded_range;

pub(crate) fn read(path: &Path, format: Format, cancel: &CancelToken) -> Details {
    let details = match format {
        Format::Pmx => pmx::read(path),
        Format::Vmd => vmd::read(path),
        Format::Vrm | Format::Gltf | Format::Glb | Format::Vrma => gltf::read(path, format),
        Format::Fbx => fbx::read(path, cancel),
        Format::Blend => blend::read(path),
        Format::Stl => stl::read(path),
        Format::Motion => document::read(path),
        Format::Sound(_) => sound::read(path),
    };
    details.unwrap_or_else(|fault| Details::Unreadable(Arc::from(fault.to_string())))
}

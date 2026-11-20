mod header;
mod nodes;
mod pixels;
mod preamble;
mod read;
mod write;

pub use header::{ColorMode, ColorModeData, FileHeader, GlobalMask, Version};
pub use preamble::Preamble;

use crate::compression::Encoded;
use crate::geometry::Rect;
use crate::info::Info;
use crate::key::Key;
use crate::layers::{Layer, Node};
use crate::pixels::Depth;
use crate::resources::Resources;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Composite {
    pub planes: Vec<Encoded>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub version: Version,
    pub width: u32,
    pub height: u32,
    pub depth: Depth,
    pub color_mode: ColorMode,
    pub color_mode_data: ColorModeData,
    pub resources: Resources,
    pub layers: Vec<Node>,
    pub merged_alpha: bool,
    pub global_mask: Option<GlobalMask>,
    pub info: Vec<Info>,
    pub composite: Composite,
}

impl Document {
    pub fn new(width: u32, height: u32, depth: Depth, color_mode: ColorMode) -> Self {
        Self {
            version: if width > Version::Psd.max_dimension()
                || height > Version::Psd.max_dimension()
            {
                Version::Psb
            } else {
                Version::Psd
            },
            width,
            height,
            depth,
            color_mode,
            color_mode_data: ColorModeData::Empty,
            resources: Resources::default(),
            layers: Vec::new(),
            merged_alpha: false,
            global_mask: None,
            info: Vec::new(),
            composite: Composite::default(),
        }
    }

    pub fn channel_count(&self) -> usize {
        self.composite.planes.len()
    }

    pub fn bounds(&self) -> Rect {
        Rect::from_size(0, 0, self.width, self.height)
    }

    pub fn find(&self, key: &[u8; 4]) -> Option<&Info> {
        self.info.iter().find(|info| info.key().0 == *key)
    }

    pub fn set_info(&mut self, info: Info) {
        let key = info.key();
        match self.info.iter_mut().find(|entry| entry.key() == key) {
            Some(slot) => *slot = info,
            None => self.info.push(info),
        }
    }

    pub fn remove_info(&mut self, key: &[u8; 4]) -> Option<Info> {
        let index = self.info.iter().position(|info| info.key() == Key(*key))?;
        Some(self.info.remove(index))
    }

    pub fn patterns(&self) -> impl Iterator<Item = &crate::pattern::Pattern> {
        self.info.iter().flat_map(|info| match info {
            Info::Patterns { patterns, .. } => patterns.as_slice(),
            _ => &[],
        })
    }

    pub fn linked_files(&self) -> impl Iterator<Item = &crate::info::LinkedFile> {
        self.info.iter().flat_map(|info| match info {
            Info::LinkedFiles(files) => files.files.as_slice(),
            _ => &[],
        })
    }

    pub fn layer_records(&self) -> Vec<&Layer> {
        let mut out = Vec::new();
        crate::layers::tree::flatten(&self.layers, &mut out);
        out
    }
}

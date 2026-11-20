use animate_cast::{RESOLUTION_PRESETS, Resolution, parse_side};
use rikka_app::prelude::*;

#[derive(Debug)]
pub(crate) struct Sizing {
    key: u64,
    opened: Resolution,
    width: TextDocument,
    height: TextDocument,
}

impl Sizing {
    pub(super) fn new(resolution: Resolution, key: u64) -> Self {
        let sizing = Self {
            key,
            opened: resolution,
            width: TextDocument::new(resolution.width().to_string()),
            height: TextDocument::new(resolution.height().to_string()),
        };
        sizing.width.select_all();
        sizing
    }

    pub(crate) fn key(&self) -> u64 {
        self.key
    }

    pub(crate) fn width(&self) -> &TextDocument {
        &self.width
    }

    pub(crate) fn height(&self) -> &TextDocument {
        &self.height
    }

    pub(crate) fn drafted(&self) -> Option<Resolution> {
        Resolution::new(
            parse_side(&self.width.text())?,
            parse_side(&self.height.text())?,
        )
    }

    pub(crate) fn shape(&self) -> Resolution {
        self.drafted().unwrap_or(self.opened)
    }

    pub(super) fn choose(&mut self, preset: usize) {
        let Some(preset) = RESOLUTION_PRESETS.get(preset) else {
            return;
        };
        let chosen = preset.facing(self.shape());
        self.width.reset(chosen.width().to_string());
        self.height.reset(chosen.height().to_string());
    }

    pub(super) fn turn(&mut self) {
        let (width, height) = (self.width.text(), self.height.text());
        self.width.reset(height);
        self.height.reset(width);
    }
}

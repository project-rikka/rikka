mod exporting;
mod sizing;

pub(crate) use exporting::{Exporting, frame};
pub(crate) use sizing::Sizing;

use animate_cast::Resolution;
use animate_export::{Encoding, Export};
use animate_preferences::Preferences;

#[derive(Debug)]
pub(crate) enum Sheet {
    Size(Sizing),
    Export(Box<Exporting>),
}

#[derive(Debug)]
pub(crate) struct Output {
    resolution: Resolution,
    encoding: Encoding,
    preferences: Preferences,
    sheet: Option<Sheet>,
    opened: u64,
}

impl Output {
    pub(crate) fn new(preferences: Preferences) -> Self {
        Self {
            resolution: preferences.resolution(),
            encoding: preferences.encoding(),
            preferences,
            sheet: None,
            opened: 0,
        }
    }

    pub(crate) fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub(crate) fn sheet(&self) -> Option<&Sheet> {
        self.sheet.as_ref()
    }

    fn sizing_mut(&mut self) -> Option<&mut Sizing> {
        match self.sheet.as_mut()? {
            Sheet::Size(sizing) => Some(sizing),
            Sheet::Export(exporting) => Some(exporting.sizing_mut()),
        }
    }

    pub(crate) fn exporting(&self) -> Option<&Exporting> {
        match self.sheet.as_ref()? {
            Sheet::Export(exporting) => Some(exporting),
            Sheet::Size(_) => None,
        }
    }

    pub(crate) fn exporting_mut(&mut self) -> Option<&mut Exporting> {
        match self.sheet.as_mut()? {
            Sheet::Export(exporting) => Some(exporting),
            Sheet::Size(_) => None,
        }
    }

    pub(crate) fn choose_preset(&mut self, preset: usize) {
        if let Some(sizing) = self.sizing_mut() {
            sizing.choose(preset);
        }
    }

    pub(crate) fn turn_sizing(&mut self) {
        if let Some(sizing) = self.sizing_mut() {
            sizing.turn();
        }
    }

    pub(crate) fn start_sizing(&mut self) {
        self.opened += 1;
        self.sheet = Some(Sheet::Size(Sizing::new(self.resolution, self.opened)));
    }

    pub(crate) fn start_exporting(&mut self, last_frame: u32, sound: bool) {
        self.opened += 1;
        self.sheet = Some(Sheet::Export(Box::new(Exporting::new(
            self.resolution,
            self.encoding,
            last_frame,
            sound,
            self.opened,
        ))));
    }

    pub(crate) fn close(&mut self) {
        self.sheet = None;
    }

    pub(crate) fn apply_sizing(&mut self) {
        let Some(Sheet::Size(sizing)) = &self.sheet else {
            return;
        };
        let Some(drafted) = sizing.drafted() else {
            return;
        };
        self.sheet = None;
        self.resize(drafted);
    }

    pub(crate) fn settle(&mut self, export: &Export) {
        self.sheet = None;
        self.resize(export.resolution);
        if self.encoding != export.encoding {
            self.encoding = export.encoding;
            self.preferences.keep_encoding(&export.encoding);
        }
    }

    fn resize(&mut self, resolution: Resolution) {
        if self.resolution != resolution {
            self.resolution = resolution;
            self.preferences.keep_resolution(resolution);
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        Self::new(Preferences::in_memory())
    }
}

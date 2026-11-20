use crate::setting::EmitterSetting;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitterLevels {
    pub rate: f32,
    pub volume: f32,
    pub speed: f32,
    pub arc: f32,
    pub width: f32,
    pub spread: f32,
}

impl Default for EmitterLevels {
    fn default() -> Self {
        Self {
            rate: 0.2,
            volume: 0.05,
            speed: 2.0,
            arc: 0.3,
            width: 0.016,
            spread: 30.0,
        }
    }
}

impl EmitterLevels {
    #[must_use]
    pub fn get(&self, setting: EmitterSetting) -> Option<f32> {
        let mut levels = *self;
        levels.slot(setting).map(|slot| *slot)
    }

    pub fn set(&mut self, setting: EmitterSetting, level: f32) {
        if let Some(slot) = self.slot(setting) {
            *slot = level;
        }
    }

    fn slot(&mut self, setting: EmitterSetting) -> Option<&mut f32> {
        match setting {
            EmitterSetting::Rate => Some(&mut self.rate),
            EmitterSetting::Volume => Some(&mut self.volume),
            EmitterSetting::Speed => Some(&mut self.speed),
            EmitterSetting::Arc => Some(&mut self.arc),
            EmitterSetting::Width => Some(&mut self.width),
            EmitterSetting::Spread => Some(&mut self.spread),
            EmitterSetting::Launch | EmitterSetting::Target => None,
        }
    }
}

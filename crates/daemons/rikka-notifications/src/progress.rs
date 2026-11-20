use serde::{Deserialize, Serialize};

const FULL: u8 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Progress {
    Indeterminate,
    Percent(u8),
}

impl Progress {
    #[must_use]
    pub fn percent(value: i32) -> Option<Progress> {
        let percent = u8::try_from(value.clamp(0, i32::from(FULL))).ok()?;
        (value >= 0).then_some(Progress::Percent(percent))
    }

    #[must_use]
    pub fn fraction(self) -> Option<f32> {
        match self {
            Progress::Indeterminate => None,
            Progress::Percent(percent) => Some(f32::from(percent) / f32::from(FULL)),
        }
    }
}

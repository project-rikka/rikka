use rikka_settings::session::displays::{AdaptiveSync, Mode};

use crate::bounds::Bounds;
use crate::orientation::Orientation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hdr {
    pub enabled: bool,
    pub sdr_brightness: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub slug: String,
    pub name: String,
    pub modes: Vec<Mode>,
    pub enabled: bool,
    pub mode: Mode,
    pub position: (i32, i32),
    pub scale: f64,
    pub orientation: Orientation,
    pub adaptive_sync: AdaptiveSync,
    pub hdr: Option<Hdr>,
}

impl Monitor {
    pub fn logical_size(&self) -> (i32, i32) {
        let width = (f64::from(self.mode.width) / self.scale).round() as i32;
        let height = (f64::from(self.mode.height) / self.scale).round() as i32;
        if self.orientation.is_upright() {
            (height.max(1), width.max(1))
        } else {
            (width.max(1), height.max(1))
        }
    }

    pub fn bounds(&self) -> Bounds {
        let (width, height) = self.logical_size();
        Bounds {
            x: self.position.0,
            y: self.position.1,
            width,
            height,
        }
    }

    pub fn resolutions(&self) -> Vec<(u32, u32)> {
        let mut resolutions: Vec<(u32, u32)> = Vec::new();
        for mode in &self.modes {
            let size = (mode.width, mode.height);
            if !resolutions.contains(&size) {
                resolutions.push(size);
            }
        }
        resolutions
    }

    pub fn refresh_rates(&self) -> Vec<Mode> {
        self.modes
            .iter()
            .copied()
            .filter(|mode| (mode.width, mode.height) == (self.mode.width, self.mode.height))
            .collect()
    }
}

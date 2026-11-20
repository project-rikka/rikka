use rikka_seam::SeamClient;
use rikka_settings::Result;
use rikka_settings::seam::Settings;
use rikka_settings::session::displays::{
    self, ADAPTIVE_SYNC, AdaptiveSync, CONNECTED, ENABLED, HDR, MODE, MODES, Mode, NAME, POSITION,
    Position, SCALE, SDR_BRIGHTNESS, SDR_BRIGHTNESS_DEFAULT, TRANSFORM,
};

use crate::monitor::{Hdr, Monitor};
use crate::orientation::Orientation;

#[derive(Debug)]
pub struct Store {
    settings: Settings,
}

impl Store {
    pub const DOMAIN: &str = displays::DOMAIN;

    pub fn adopt(seam: SeamClient) -> Result<Self> {
        Settings::adopt_with_seam(Self::DOMAIN, seam).map(|settings| Self { settings })
    }

    pub fn monitors(&self) -> Vec<Monitor> {
        let connected = self.settings.get::<String>(CONNECTED).unwrap_or_default();
        displays::list(&connected)
            .filter_map(|slug| self.monitor(slug))
            .collect()
    }

    pub fn write(&mut self, current: &Monitor, staged: &Monitor) -> Result<()> {
        let slug = &staged.slug;
        let mut refused = None;
        if staged.position != current.position {
            let (x, y) = staged.position;
            let position = Position { x, y }.to_string();
            refused = refused.or(self.write_text(slug, POSITION, &position).err());
        }
        if staged.mode != current.mode {
            refused = refused.or(self.write_text(slug, MODE, &staged.mode.to_string()).err());
        }
        if staged.orientation != current.orientation {
            let name = staged.orientation.name();
            refused = refused.or(self.write_text(slug, TRANSFORM, name).err());
        }
        if staged.scale != current.scale {
            refused = refused.or(self.write_float(slug, SCALE, staged.scale).err());
        }
        if staged.enabled != current.enabled {
            refused = refused.or(self.write_bool(slug, ENABLED, staged.enabled).err());
        }
        if staged.adaptive_sync != current.adaptive_sync {
            let key = displays::key(slug, ADAPTIVE_SYNC);
            let adaptive_sync = staged.adaptive_sync;
            refused = refused.or(self.settings.set_choice(&key, adaptive_sync).err());
        }
        if let (Some(staged), Some(current)) = (staged.hdr, current.hdr) {
            if staged.enabled != current.enabled {
                refused = refused.or(self.write_bool(slug, HDR, staged.enabled).err());
            }
            if staged.sdr_brightness != current.sdr_brightness {
                let nits = staged.sdr_brightness;
                refused = refused.or(self.write_float(slug, SDR_BRIGHTNESS, nits).err());
            }
        }
        refused.map_or(Ok(()), Err)
    }

    pub fn write_sdr_brightness(&mut self, slug: &str, nits: f64) -> Result<()> {
        self.write_float(slug, SDR_BRIGHTNESS, nits)
    }

    fn monitor(&self, slug: &str) -> Option<Monitor> {
        let modes = Mode::parse_all(&self.text(slug, MODES)?);
        let mode = Mode::parse(&self.text(slug, MODE)?)?;
        let position = Position::parse(&self.text(slug, POSITION)?)?;
        Some(Monitor {
            slug: slug.to_owned(),
            name: self.text(slug, NAME)?,
            modes,
            enabled: self.settings.get(&displays::key(slug, ENABLED)).ok()?,
            mode,
            position: (position.x, position.y),
            scale: self.settings.get(&displays::key(slug, SCALE)).ok()?,
            orientation: Orientation::parse(&self.text(slug, TRANSFORM)?)?,
            adaptive_sync: self
                .settings
                .choice::<AdaptiveSync>(&displays::key(slug, ADAPTIVE_SYNC)),
            hdr: self.hdr(slug),
        })
    }

    fn hdr(&self, slug: &str) -> Option<Hdr> {
        let enabled = self.settings.get(&displays::key(slug, HDR)).ok()?;
        Some(Hdr {
            enabled,
            sdr_brightness: self
                .settings
                .get(&displays::key(slug, SDR_BRIGHTNESS))
                .unwrap_or(SDR_BRIGHTNESS_DEFAULT),
        })
    }

    fn text(&self, slug: &str, suffix: &str) -> Option<String> {
        self.settings.get(&displays::key(slug, suffix)).ok()
    }

    fn write_text(&mut self, slug: &str, suffix: &str, value: &str) -> Result<()> {
        self.settings.set(&displays::key(slug, suffix), value)
    }

    fn write_bool(&mut self, slug: &str, suffix: &str, value: bool) -> Result<()> {
        self.settings.set(&displays::key(slug, suffix), value)
    }

    fn write_float(&mut self, slug: &str, suffix: &str, value: f64) -> Result<()> {
        self.settings.set(&displays::key(slug, suffix), value)
    }
}

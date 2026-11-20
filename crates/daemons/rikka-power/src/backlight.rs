use std::path::{Path, PathBuf};

use zbus::blocking::Connection;
use zbus::zvariant::OwnedObjectPath;

use crate::error::{Error, Result};
use rikka_bus::login::{self, SessionProxy};

const SUBSYSTEM: &str = "backlight";

const CLASS: &str = "/sys/class/backlight";

pub(crate) struct Backlight {
    session: SessionProxy<'static>,
    name: String,
    device: PathBuf,
    max: u32,
}

impl Backlight {
    pub(crate) fn level(&self) -> Result<f32> {
        let current = read(&self.device.join("brightness"))?;
        Ok(current as f32 / self.max as f32)
    }

    pub(crate) fn set_level(&self, level: f32) -> Result<()> {
        self.session
            .set_brightness(SUBSYSTEM, &self.name, steps(level, self.max))?;
        Ok(())
    }

    pub(crate) fn found(connection: &Connection, session: OwnedObjectPath) -> Option<Backlight> {
        let device = best(Path::new(CLASS))?;
        let name = device.file_name()?.to_str()?.to_owned();
        let max = read(&device.join("max_brightness"))
            .ok()
            .filter(|max| *max > 0)?;
        Some(Backlight {
            session: login::session_proxy(connection, session).ok()?,
            name,
            device,
            max,
        })
    }
}

fn steps(level: f32, max: u32) -> u32 {
    let raw = (level.clamp(0.0, 1.0) * max as f32).round() as u32;
    raw.clamp(1, max)
}

fn best(class: &Path) -> Option<PathBuf> {
    let mut devices: Vec<(u8, PathBuf)> = std::fs::read_dir(class)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .map(|device| (rank(&device), device))
        .collect();
    devices.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    devices.into_iter().next().map(|(_, device)| device)
}

fn rank(device: &Path) -> u8 {
    match std::fs::read_to_string(device.join("type"))
        .unwrap_or_default()
        .trim()
    {
        "firmware" => 0,
        "platform" => 1,
        _ => 2,
    }
}

fn read(file: &Path) -> Result<u32> {
    std::fs::read_to_string(file)
        .map_err(|error| Error::Backlight(format!("{}: {error}", file.display())))?
        .trim()
        .parse()
        .map_err(|error| Error::Backlight(format!("{}: {error}", file.display())))
}

impl std::fmt::Debug for Backlight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backlight")
            .field("name", &self.name)
            .field("max", &self.max)
            .finish_non_exhaustive()
    }
}

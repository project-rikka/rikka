mod engine;
mod signals;
mod sound;
mod video;
mod voice;

use std::path::Path;
use std::time::Duration;

pub(crate) use sound::Sound;
pub(crate) use video::Video;
pub(crate) use voice::Settings;

fn check_file(path: &Path) -> Result<(), String> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Err(format!("{} is a folder", path.display())),
        Ok(_) => Ok(()),
        Err(error) => Err(format!("failed to open {}: {error}", path.display())),
    }
}

fn clamp_to(position: Duration, duration: Option<Duration>) -> Duration {
    duration.map_or(position, |duration| position.min(duration))
}

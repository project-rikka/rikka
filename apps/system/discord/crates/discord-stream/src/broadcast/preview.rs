use std::time::{Duration, Instant};

use rikka_image::{Image, Jpeg};
use rikka_screencast::{PixelOrder, Pixels};

const FIRST: Duration = Duration::from_secs(3);
const THUMBNAIL_INTERVAL: Duration = Duration::from_millis(200);
const EVERY: Duration = Duration::from_secs(300);
const QUALITY: u8 = 80;

pub(crate) const SIDE: (u32, u32) = (640, 360);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shot {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct Preview {
    due: Instant,
    last_thumbnail: Option<Instant>,
}

impl Preview {
    pub(crate) fn new(started: Instant) -> Preview {
        Preview {
            due: started + FIRST,
            last_thumbnail: None,
        }
    }

    pub(crate) fn thumbnail_due(&mut self, now: Instant) -> bool {
        if self.last_thumbnail.is_some_and(|last_thumbnail| {
            now.saturating_duration_since(last_thumbnail) < THUMBNAIL_INTERVAL
        }) {
            return false;
        }
        self.last_thumbnail = Some(now);
        true
    }

    pub(crate) fn upload_due(&mut self, now: Instant) -> bool {
        if now < self.due {
            return false;
        }
        self.due = now + EVERY;
        true
    }
}

pub(crate) fn shrink(
    pixels: &Pixels,
    (source_width, source_height): (u32, u32),
    (most_width, most_height): (u32, u32),
) -> Shot {
    let scale = (f64::from(most_width) / f64::from(source_width.max(1)))
        .min(f64::from(most_height) / f64::from(source_height.max(1)))
        .min(1.0);
    let width = ((f64::from(source_width) * scale).round() as u32).max(1);
    let height = ((f64::from(source_height) * scale).round() as u32).max(1);
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        let from_y = (u64::from(y) * u64::from(source_height) / u64::from(height)) as usize;
        let line = from_y * pixels.stride;
        for x in 0..width {
            let from_x = (u64::from(x) * u64::from(source_width) / u64::from(width)) as usize;
            let at = line + from_x * 4;
            let Some(pixel) = pixels.bytes.get(at..at + 4) else {
                rgba.extend_from_slice(&[0, 0, 0, 255]);
                continue;
            };
            match pixels.order {
                PixelOrder::Bgra => rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]),
                PixelOrder::Rgba => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
            }
        }
    }
    Shot {
        width,
        height,
        rgba,
    }
}

pub(crate) fn jpeg(shot: Shot) -> rikka_image::Result<Vec<u8>> {
    Image::from_rgba8(shot.width, shot.height, shot.rgba)?
        .write(Jpeg)
        .quality(QUALITY)
        .to_bytes()
}

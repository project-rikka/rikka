use std::sync::Arc;
use std::time::Duration;

use discord_core::Picture;
use rikka_app::prelude::*;
use rikka_ui::ImageSource;

use crate::conversation::widgets::{Frames, Still};
use crate::pictures;

const MAX_FRAMES: usize = 240;
const MAX_BYTES: usize = 64 << 20;
const SHORTEST_DELAY: Duration = Duration::from_millis(20);
const BLUR: u32 = 12;

pub(crate) fn frames(picture: &Picture, side: f32, bound: Size) -> Option<Arc<Frames>> {
    let bytes = pictures::fetched(picture, side)?;
    match decoded_frames(picture, bytes, bound) {
        Fetch::Ready(frames) => Some(frames),
        _ => None,
    }
}

pub(crate) fn decoded_frames(picture: &Picture, bytes: Arc<[u8]>, bound: Size) -> Fetch<Frames> {
    let (width, height) = (bound.width.ceil() as u32, bound.height.ceil() as u32);
    fetch_blocking(
        ("discord-frames", picture.url().to_owned(), width, height),
        move || decode(&bytes, width.max(1) * 2, height.max(1) * 2),
    )
}

fn decode(bytes: &[u8], width: u32, height: u32) -> rikka_image::Result<Frames> {
    let animation = rikka_image::read(bytes).animation()?;
    let size = animation.size();
    let mut shown = Vec::new();
    let mut spent = 0usize;
    for frame in animation.into_frames().into_iter().take(MAX_FRAMES) {
        let delay = frame.delay().max(SHORTEST_DELAY);
        let still = frame.into_image().thumbnail(width, height)?;
        let (wide, tall) = still.dimensions();
        let pixels: Arc<[u8]> = Arc::from(still.into_rgba8());
        spent += pixels.len();
        shown.push(Still {
            width: wide,
            height: tall,
            pixels,
            delay,
        });
        if spent > MAX_BYTES {
            break;
        }
    }
    Ok(Frames { shown, size })
}

pub(crate) fn blurred(picture: &Picture) -> Option<ImageSource> {
    let bytes = pictures::fetched(picture, 64.0)?;
    let decoded = fetch_blocking(("discord-blurred", picture.url().to_owned()), move || {
        shrink(&bytes)
    });
    match decoded {
        Fetch::Ready(still) => Some(still.source()),
        _ => None,
    }
}

fn shrink(bytes: &[u8]) -> rikka_image::Result<Still> {
    let still = rikka_image::read(bytes).decode()?.thumbnail(BLUR, BLUR)?;
    let (width, height) = still.dimensions();
    let pixels: Vec<u8> = still.into_rgba8();
    Ok(Still {
        width,
        height,
        pixels: Arc::from(pixels),
        delay: Duration::ZERO,
    })
}

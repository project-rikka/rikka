use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_image::{Format, ImageInfo};
use rikka_ui::ImageSource;

use super::Shot;
use crate::conversation::media::decoded_frames;
use crate::conversation::widgets::{Frames, Still};
use crate::pictures;

pub(super) const LOADED_AT: f32 = 2048.0;
const PREVIEW_AT: f32 = 400.0;
const LARGEST: u32 = 4096;
const MOVING_BOUND: f32 = 1024.0;

#[derive(Clone, Debug)]
pub(super) enum Face {
    Waiting,
    Failed,
    Still(ImageSource),
    Moving(Arc<Frames>),
}

#[derive(Clone, Debug)]
pub(super) struct Look {
    pub(super) face: Face,
    pub(super) under: Option<ImageSource>,
    pub(super) natural: Option<Size>,
}

impl Look {
    fn new(face: Face, under: Option<ImageSource>, natural: Option<Size>) -> Look {
        let drawn = under.is_some() || matches!(face, Face::Still(_) | Face::Moving(_));
        Look {
            face,
            under,
            natural: natural.filter(|_| drawn),
        }
    }
}

pub(super) fn look(shot: &Shot) -> Look {
    let preview = shot
        .preview
        .as_ref()
        .filter(|preview| **preview != shot.full)
        .and_then(|preview| pictures::fetched(preview, PREVIEW_AT))
        .map(ImageSource::Bytes);
    let stated = (shot.width > 0 && shot.height > 0)
        .then(|| Size::new(shot.width as f32, shot.height as f32));
    let waiting = |face: Face| Look::new(face, preview.clone(), stated);
    let bytes = match pictures::fetching(&shot.full, LOADED_AT) {
        Fetch::Loading => return waiting(Face::Waiting),
        Fetch::Failed(_) => return waiting(Face::Failed),
        Fetch::Ready(bytes) => Arc::clone(&*bytes),
    };
    let info = match probed(shot, &bytes) {
        Fetch::Loading => return waiting(Face::Waiting),
        Fetch::Failed(_) => return waiting(Face::Failed),
        Fetch::Ready(info) => info,
    };
    let face = match () {
        _ if shot.animated || info.format == Format::Gif => {
            match decoded_frames(&shot.full, Arc::clone(&bytes), moving_bound()) {
                Fetch::Ready(frames) => Face::Moving(frames),
                Fetch::Loading => Face::Waiting,
                Fetch::Failed(_) => Face::Still(ImageSource::Bytes(Arc::clone(&bytes))),
            }
        }
        _ if info.width.max(info.height) > LARGEST => match shrunk(shot, &bytes) {
            Fetch::Ready(still) => Face::Still(still.source()),
            Fetch::Loading => Face::Waiting,
            Fetch::Failed(_) => Face::Failed,
        },
        _ => Face::Still(ImageSource::Bytes(Arc::clone(&bytes))),
    };
    let natural = match &face {
        Face::Moving(frames) => frames.size,
        _ => info.size(),
    };
    let under = match (&face, info.has_alpha()) {
        (Face::Waiting, _) | (_, false) => preview,
        _ => None,
    };
    Look::new(face, under, Some(natural))
}

fn moving_bound() -> Size {
    Size::new(MOVING_BOUND, MOVING_BOUND)
}

fn probed(shot: &Shot, bytes: &Arc<[u8]>) -> Fetch<ImageInfo> {
    let bytes = Arc::clone(bytes);
    fetch_blocking(
        ("discord-lightbox-probe", shot.full.url().to_owned()),
        move || rikka_image::read(&bytes[..]).probe(),
    )
}

fn shrunk(shot: &Shot, bytes: &Arc<[u8]>) -> Fetch<Still> {
    let bytes = Arc::clone(bytes);
    fetch_blocking(
        ("discord-lightbox-shrunk", shot.full.url().to_owned()),
        move || {
            let image = rikka_image::read(&bytes[..])
                .decode()?
                .thumbnail(LARGEST, LARGEST)?;
            let (width, height) = image.dimensions();
            Ok::<_, rikka_image::Error>(Still {
                width,
                height,
                pixels: Arc::from(image.into_rgba8()),
                delay: std::time::Duration::ZERO,
            })
        },
    )
}

use std::path::Path;
use std::sync::Arc;

use cut_filmstrip::{Filmstrips, Footage, Mode, Request, Thumbnail, Window, decoded_height, tiles};
use cut_project::{Clip, Media, Rate};
use rikka_app::prelude::*;

use crate::timeline::scale::Scale;

const AHEAD: f32 = 0.5;

#[derive(Debug, Default)]
pub(super) struct Requests {
    now: Vec<Request>,
    soon: Vec<Request>,
}

impl Requests {
    pub(super) fn submit(self, filmstrips: &Filmstrips) {
        let mut requests = self.now;
        requests.extend(self.soon);
        filmstrips.set_requests(requests);
    }
}

pub(super) struct Strip<'a> {
    pub(super) filmstrips: Option<&'a Filmstrips>,
    pub(super) mode: Mode,
    pub(super) rate: Rate,
    pub(super) scale: &'a Scale,
    pub(super) bounds: Rect,
    pub(super) visible: Rect,
    pub(super) resolution: f32,
}

impl Strip<'_> {
    pub(super) fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        requests: &mut Requests,
        clip: &Clip,
        media: &Media,
        band: Rect,
    ) {
        let (Some(filmstrips), Some(picture)) = (self.filmstrips, media.picture()) else {
            return;
        };
        if band.max_y() <= self.visible.min_y() || band.min_y() >= self.visible.max_y() {
            return;
        }
        let aspect = picture.width.max(1) as f32 / picture.height.max(1) as f32;
        let thumbnail_width = band.height() * aspect;
        let footage = Footage {
            start: clip.start(),
            end: clip.end(),
            source_in: clip.source_in(),
            speed: clip.speed(),
            last: media.frames(self.rate) - 1,
        };
        let width = self.bounds.width();
        let window = Window {
            first: self.scale.frame_at(-width * AHEAD),
            per_frame: self.scale.per_frame(),
            width: width * (1.0 + AHEAD * 2.0),
        };
        let path: Arc<Path> = Arc::from(media.path());
        let height = decoded_height(band.height() * self.resolution);
        let shift = self.bounds.x() - width * AHEAD;
        for tile in tiles(self.mode, footage, window, thumbnail_width) {
            let request = Request {
                path: Arc::clone(&path),
                at: self.rate.start_of(tile.source),
                tolerance: self.rate.start_of(tile.tolerance),
                height,
            };
            let span = Rect::new(
                shift + tile.left,
                band.y(),
                tile.right - tile.left,
                band.height(),
            );
            let visible = span.intersects(&self.visible);
            let matched = filmstrips.find(&request);
            if visible && let Some(matched) = &matched {
                paint_thumbnail(ctx, &matched.thumbnail, span, band, tile.tail);
            }
            if matched.is_none_or(|matched| !matched.exact) {
                match visible {
                    true => requests.now.push(request),
                    false => requests.soon.push(request),
                }
            }
        }
    }
}

fn paint_thumbnail(
    ctx: &mut PaintCtx<'_>,
    thumbnail: &Thumbnail,
    span: Rect,
    band: Rect,
    tail: bool,
) {
    let Some(within) = span.intersection(&band) else {
        return;
    };
    let ratio = thumbnail.width.max(1) as f32 / thumbnail.height.max(1) as f32;
    let natural = band.height() * ratio;
    let (width, height) = match span.width() > natural {
        true => (span.width(), span.width() / ratio),
        false => (natural, band.height()),
    };
    let left = match tail {
        true => span.max_x() - width,
        false => span.x(),
    };
    let top = band.center().y - height / 2.0;
    let source = ImageSource::Rgba {
        width: thumbnail.width,
        height: thumbnail.height,
        pixels: Arc::clone(&thumbnail.pixels),
    };
    ctx.clipped(within, |ctx| {
        ctx.draw_image(Rect::new(left, top, width, height), source);
    });
}

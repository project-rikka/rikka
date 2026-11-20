use cut_project::{Clip, Edge, Fade, Media, Mix, MixField, Property, Stream};
use rikka_app::prelude::*;

use crate::timeline::rows::Row;
use crate::timeline::scale::Scale;

pub(super) const INSET: f32 = 2.0;
pub(super) const HEADER: f32 = 16.0;
pub(super) const HANDLE: f32 = 7.0;
const MIN_CONTENT: f32 = 12.0;
const SPLIT_FROM: f32 = 34.0;
const PICTURE_SHARE: f32 = 0.58;
const LEVEL_PAD: f32 = 3.0;
const LOUDEST: f32 = 2.0;

pub(super) struct Parts {
    pub(super) header: Rect,
    pub(super) picture: Option<Rect>,
    pub(super) sound: Option<Rect>,
}

impl Parts {
    pub(super) fn layout(body: Rect, media: Option<&Media>) -> Parts {
        let whole = Parts {
            header: body,
            picture: None,
            sound: None,
        };
        let Some(media) = media else {
            return whole;
        };
        let content = body.height() - HEADER;
        if content < MIN_CONTENT {
            return whole;
        }
        let header = Rect::new(body.x(), body.y(), body.width(), HEADER);
        let below = Rect::new(body.x(), body.y() + HEADER, body.width(), content);
        let (picture, sound) = match (media.has(Stream::Picture), media.has(Stream::Sound)) {
            (true, true) if content >= SPLIT_FROM => {
                let picture_height = (content * PICTURE_SHARE).round();
                (
                    Some(Rect::new(
                        below.x(),
                        below.y(),
                        below.width(),
                        picture_height,
                    )),
                    Some(Rect::new(
                        below.x(),
                        below.y() + picture_height,
                        below.width(),
                        content - picture_height,
                    )),
                )
            }
            (true, _) => (Some(below), None),
            (false, true) => (None, Some(below)),
            (false, false) => return whole,
        };
        Parts {
            header,
            picture,
            sound,
        }
    }
}

pub(super) fn body(scale: &Scale, row: &Row, from: i64, to: i64) -> Rect {
    let left = scale.x_of(from as f64);
    let right = scale.x_of(to as f64);
    Rect::new(
        left,
        row.top + INSET,
        (right - left).max(1.0),
        row.height - INSET * 2.0,
    )
}

pub(super) fn level_y(sound: Rect, volume: f32) -> f32 {
    let reach = (sound.height() - LEVEL_PAD * 2.0).max(0.0);
    sound.max_y() - LEVEL_PAD - reach * raised(volume)
}

pub(super) fn volume_at(sound: Rect, y: f32) -> f32 {
    let reach = (sound.height() - LEVEL_PAD * 2.0).max(f32::EPSILON);
    lowered((sound.max_y() - LEVEL_PAD - y) / reach)
}

pub(super) fn level_keys(
    scale: &Scale,
    clip: &Clip,
    sound: Rect,
) -> impl Iterator<Item = (i64, Point)> {
    let channel = clip.channel(&Property::Mix(MixField::Volume));
    channel
        .map(|channel| channel.frames())
        .unwrap_or_default()
        .into_iter()
        .filter_map(move |at| {
            let value = channel?.value(at)?.scalar()?;
            let frame = clip.start() + at;
            Some((
                frame,
                Point::new(scale.x_of(frame as f64), level_y(sound, value)),
            ))
        })
}

pub(super) fn raised(volume: f32) -> f32 {
    (volume.max(0.0) / LOUDEST).sqrt().min(1.0)
}

pub(super) fn lowered(raised: f32) -> f32 {
    let raised = raised.clamp(0.0, 1.0);
    LOUDEST * raised * raised
}

pub(super) fn fade_handle(scale: &Scale, clip: &Clip, sound: Rect, mix: Mix, edge: Edge) -> Rect {
    let (frame, fade): (i64, Fade) = match edge {
        Edge::Head => (clip.start() + mix.fade_in.length, mix.fade_in),
        Edge::Tail => (clip.end() - mix.fade_out.length, mix.fade_out),
    };
    let x = scale.x_of(frame as f64);
    let x = match (edge, fade.is_on()) {
        (Edge::Head, false) => x + HANDLE / 2.0,
        (Edge::Tail, false) => x - HANDLE / 2.0,
        (_, true) => x,
    };
    Rect::from_center(
        Point::new(x, sound.y() + HANDLE / 2.0 + 1.0),
        Size::new(HANDLE, HANDLE),
    )
}

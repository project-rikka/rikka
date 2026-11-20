use cut_project::{Channel, Clip, ClipId, Edit, Handle, KeyChange, KeyRef, Property, Side, Span};
use rikka_app::prelude::*;

const SAMPLES: usize = 48;
const FLAT: f64 = 1e-6;
const THIRD: f32 = 1.0 / 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HandleChange {
    Speed(f32),
    Influence(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Segment {
    property: Property,
    from: i64,
    to: i64,
    start: f64,
    frames: f64,
    base: f64,
    scale: f64,
    end: f32,
    handles: [Point; 2],
    curve: Vec<Point>,
    curved: [bool; 2],
}

impl Segment {
    pub(crate) fn for_key(clip: &Clip, key: &KeyRef) -> Option<Segment> {
        let channel = clip.channel(&key.property)?;
        let start = (key.at - clip.start()) as f64;
        let span = channel
            .spans()
            .into_iter()
            .find(|span| span.from[0] == start)?;
        Some(Segment::spanning(&channel, &span, key, clip.start()))
    }

    fn spanning(channel: &Channel<'_>, span: &Span, key: &KeyRef, offset: i64) -> Segment {
        let frames = span.to[0] - span.from[0];
        let travel = span.to[1] - span.from[1];
        let scale = match travel.abs() < FLAT {
            true => 1.0,
            false => travel,
        };
        let normal = |[x, y]: [f64; 2]| {
            Point::new(
                ((x - span.from[0]) / frames) as f32,
                ((y - span.from[1]) / scale) as f32,
            )
        };
        let end = (travel / scale) as f32;
        let (out, into) = (span.outgoing.map(normal), span.incoming.map(normal));
        let out_reach = THIRD.min(into.map_or(1.0, |into| into.x));
        let into_reach = THIRD.min(1.0 - out.map_or(0.0, |out| out.x));
        let curve = (0..=SAMPLES)
            .filter_map(|step| {
                let along = step as f64 / SAMPLES as f64;
                let value = channel.graph_value(span.from[0] + along * frames)?;
                Some(normal([span.from[0] + along * frames, value]))
            })
            .collect();
        Segment {
            property: key.property.clone(),
            from: key.at,
            to: offset + span.to[0] as i64,
            start: span.from[0],
            frames,
            base: span.from[1],
            scale,
            end,
            handles: [
                out.unwrap_or(Point::new(out_reach, out_reach * end)),
                into.unwrap_or(Point::new(1.0 - into_reach, end * (1.0 - into_reach))),
            ],
            curve,
            curved: [out.is_some(), into.is_some()],
        }
    }

    pub(crate) fn end(&self) -> f32 {
        self.end
    }

    pub(crate) fn handles(&self) -> [Point; 2] {
        self.handles
    }

    pub(crate) fn curve(&self) -> &[Point] {
        &self.curve
    }

    pub(crate) fn handle(&self, side: Side) -> Handle {
        let [out, into] = self.handles;
        let (reach, rise) = match side {
            Side::Outgoing => (out.x, out.y),
            Side::Incoming => (1.0 - into.x, self.end - into.y),
        };
        let across = f64::from(reach) * self.frames;
        let speed = match across > FLAT {
            true => f64::from(rise) * self.scale / across,
            false => 0.0,
        };
        Handle::new(speed as f32, reach)
    }

    fn key_at(&self, side: Side) -> i64 {
        match side {
            Side::Outgoing => self.from,
            Side::Incoming => self.to,
        }
    }

    fn edit(&self, clip: ClipId, side: Side, handle: Handle) -> Edit {
        Edit::Key {
            clip,
            property: self.property.clone(),
            change: KeyChange::Handle {
                at: self.key_at(side),
                side,
                handle,
            },
        }
    }

    pub(crate) fn handle_edit(&self, clip: ClipId, side: Side, change: HandleChange) -> Edit {
        let current = self.handle(side);
        let handle = match change {
            HandleChange::Speed(speed) => Handle::new(speed, current.influence),
            HandleChange::Influence(influence) => Handle::new(current.speed, influence),
        };
        self.edit(clip, side, handle)
    }

    pub(crate) fn dragged(&self, clip: &Clip, side: Side, direction: Point) -> Vec<Edit> {
        let Some(channel) = clip.channel(&self.property) else {
            return Vec::new();
        };
        let handle = |side: Side, direction: Point| {
            let point = [
                self.start + f64::from(direction.x) * self.frames,
                self.base + f64::from(direction.y) * self.scale,
            ];
            channel.handle_toward(self.key_at(side) - clip.start(), side, point)
        };
        let mut edits = Vec::new();
        if side == Side::Incoming
            && !self.curved[0]
            && let Some(outgoing) = handle(Side::Outgoing, self.handles[0])
        {
            edits.push(self.edit(clip.id(), Side::Outgoing, outgoing));
        }
        if let Some(changed) = handle(side, direction) {
            edits.push(self.edit(clip.id(), side, changed));
        }
        edits
    }
}

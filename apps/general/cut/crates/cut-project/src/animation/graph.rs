use rikka_types::Point;

use super::key::{Handle, Incoming, Outgoing, Side};
use super::kind::{Kind, Spatial};
use super::leg::{Shape, handles};
use super::route::bezier;
use super::track::Track;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub outgoing: Option<[f64; 2]>,
    pub incoming: Option<[f64; 2]>,
    pub hold: bool,
}

impl<K: Kind> Track<K> {
    #[must_use]
    pub fn graph_value(&self, at: f64) -> Option<f64> {
        let (first, last) = (self.keys().first()?, self.keys().last()?);
        let bases = self.bases();
        if at <= first.at as f64 {
            return bases.first().copied();
        }
        if at >= last.at as f64 {
            return bases.last().copied();
        }
        let index = self.leg_at(at);
        Some(bases[index] + self.leg(index).travel(at - self.keys()[index].at as f64))
    }

    #[must_use]
    pub fn spans(&self) -> Vec<Span> {
        let bases = self.bases();
        self.keys()
            .windows(2)
            .enumerate()
            .map(|(index, pair)| {
                let (start, end) = (pair[0].at as f64, pair[1].at as f64);
                let leg = self.leg(index);
                let base = bases[index];
                let (outgoing, incoming) = match leg.shape {
                    Shape::Curve { xs, ys } => (
                        matches!(pair[0].timing.outgoing, Outgoing::Bezier(_))
                            .then_some([start + xs[1], base + ys[1]]),
                        matches!(pair[1].timing.incoming, Incoming::Bezier(_))
                            .then_some([start + xs[2], base + ys[2]]),
                    ),
                    Shape::Hold | Shape::Preset(_) => (None, None),
                };
                Span {
                    from: [start, base],
                    to: [end, bases[index + 1]],
                    outgoing,
                    incoming,
                    hold: matches!(leg.shape, Shape::Hold),
                }
            })
            .collect()
    }

    #[must_use]
    pub fn handle_toward(&self, at: i64, side: Side, point: [f64; 2]) -> Option<Handle> {
        let index = self.find(at).ok()?;
        let bases = self.bases();
        let (leg, sign) = match side {
            Side::Outgoing => (index, 1.0),
            Side::Incoming => (index.checked_sub(1)?, -1.0),
        };
        let pair = self.keys().get(leg..=leg + 1)?;
        let frames = (pair[1].at - pair[0].at) as f64;
        let least = f64::from(Handle::MIN_INFLUENCE) * frames;
        let across = (sign * (point[0] - at as f64)).clamp(least, frames);
        let rise = sign * (point[1] - bases[index]);
        let other = match side {
            Side::Outgoing => pair[1].timing.handle(Side::Incoming),
            Side::Incoming => pair[0].timing.handle(Side::Outgoing),
        };
        let room = other.map_or(1.0, |other| 1.0 - other.influence);
        let influence = ((across / frames) as f32).min(room);
        Some(Handle::new((rise / across) as f32, influence).checked())
    }

    pub(crate) fn effective(&self, index: usize) -> Option<[Handle; 2]> {
        let pair = self.keys().get(index..=index + 1)?;
        let leg = self.leg(index);
        let chord = leg.route.length() / leg.frames;
        let [out, into] = handles(pair[0].timing.outgoing, pair[1].timing.incoming, chord);
        let handle = |(speed, influence): (f64, f64)| Handle::new(speed as f32, influence as f32);
        Some([handle(out), handle(into)])
    }
}

impl Track<Spatial> {
    #[must_use]
    pub fn path(&self) -> Vec<[Point; 4]> {
        self.keys()
            .windows(2)
            .filter_map(|pair| bezier(&pair[0], &pair[1]))
            .map(|points| points.map(|[x, y]| Point::new(x as f32, y as f32)))
            .collect()
    }
}

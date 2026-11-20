use super::cubic::{self, Cubic};
use super::key::{Handle, Incoming, Key, Outgoing, Preset};
use super::kind::Kind;
use super::route::Route;

const THIRD: f64 = 1.0 / 3.0;
const LEAST: f64 = Handle::MIN_INFLUENCE as f64;
const NUDGE: f64 = 1e-4;
const FLAT: f64 = 1e-12;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Shape {
    Hold,
    Curve { xs: Cubic, ys: Cubic },
    Preset(Preset),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Leg {
    pub(crate) frames: f64,
    pub(crate) route: Route,
    pub(crate) shape: Shape,
}

impl Leg {
    pub(crate) fn between<K: Kind>(from: &Key<K>, to: &Key<K>) -> Leg {
        let frames = (to.at - from.at) as f64;
        let route = Route::between(from, to);
        let travel = route.length();
        let shape = match from.timing.outgoing {
            Outgoing::Hold => Shape::Hold,
            Outgoing::Preset(preset) => Shape::Preset(preset),
            Outgoing::Linear | Outgoing::Bezier(_) => {
                let [(out_speed, out), (in_speed, into)] =
                    handles(from.timing.outgoing, to.timing.incoming, travel / frames);
                Shape::Curve {
                    xs: [0.0, out * frames, frames - into * frames, frames],
                    ys: [
                        0.0,
                        out_speed * out * frames,
                        travel - in_speed * into * frames,
                        travel,
                    ],
                }
            }
        };
        Leg {
            frames,
            route,
            shape,
        }
    }

    pub(crate) fn travel(&self, at: f64) -> f64 {
        let length = self.route.length();
        match self.shape {
            Shape::Hold if at < self.frames => 0.0,
            Shape::Hold => length,
            Shape::Curve { xs, ys } => cubic::at(ys, cubic::solve(xs, at)),
            Shape::Preset(preset) => length * eased(preset, at / self.frames),
        }
    }

    pub(crate) fn speed(&self, at: f64) -> f64 {
        match self.shape {
            Shape::Hold => 0.0,
            Shape::Curve { xs, ys } => {
                let u = cubic::solve(xs, at.clamp(0.0, self.frames));
                let across = cubic::slope(xs, u);
                match across > FLAT {
                    true => cubic::slope(ys, u) / across,
                    false => 0.0,
                }
            }
            Shape::Preset(preset) => {
                let progress = (at / self.frames).clamp(0.0, 1.0);
                let (before, after) = ((progress - NUDGE).max(0.0), (progress + NUDGE).min(1.0));
                self.route.length() * (eased(preset, after) - eased(preset, before))
                    / ((after - before) * self.frames)
            }
        }
    }

    pub(crate) fn value<K: Kind>(&self, from: &Key<K>, to: &Key<K>, at: f64) -> K::Value {
        if at <= 0.0 {
            return from.value;
        }
        if at >= self.frames {
            return to.value;
        }
        match self.shape {
            Shape::Hold => from.value,
            Shape::Curve { .. } | Shape::Preset(_) => {
                K::from_coords(self.route.at(self.travel(at)))
            }
        }
    }
}

pub(crate) fn handles(outgoing: Outgoing, incoming: Incoming, chord: f64) -> [(f64, f64); 2] {
    let out = match outgoing {
        Outgoing::Bezier(handle) => Some(handle),
        Outgoing::Hold | Outgoing::Linear | Outgoing::Preset(_) => None,
    };
    let into = match incoming {
        Incoming::Bezier(handle) => Some(handle),
        Incoming::Linear => None,
    };
    let pair = |handle: Handle| (f64::from(handle.speed), f64::from(handle.influence));
    let linear = |other: f64| (chord, THIRD.min(1.0 - other).max(LEAST));
    match (out.map(pair), into.map(pair)) {
        (Some((out_speed, out)), Some((in_speed, into))) => {
            let scale = 1.0 / (out + into).max(1.0);
            [(out_speed, out * scale), (in_speed, into * scale)]
        }
        (Some(out), None) => [out, linear(out.1)],
        (None, Some(into)) => [linear(into.1), into],
        (None, None) => [(chord, THIRD), (chord, THIRD)],
    }
}

pub(crate) fn eased(preset: Preset, progress: f64) -> f64 {
    let (from, to) = (f64::from(preset.from), f64::from(preset.to));
    let curve = |x: f64| preset.easing.at(x);
    let rise = curve(to) - curve(from);
    let progress = progress.clamp(0.0, 1.0);
    match rise.abs() > FLAT {
        true => (curve(from + (to - from) * progress) - curve(from)) / rise,
        false => progress,
    }
}

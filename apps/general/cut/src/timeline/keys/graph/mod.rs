mod handle_drag;
mod paint;
pub(crate) mod points;

pub(crate) use handle_drag::{HandleDrag, Target, banded, hit};
pub(crate) use paint::Drawing;

use std::collections::BTreeSet;

use cut_project::{Channel, Clip, Property, Side};
use rikka_app::prelude::*;

use super::outline::Outline;
use crate::cut::Plot;

const PAD: f32 = 12.0;
const SAMPLES: i64 = 240;
const MARGIN: f64 = 0.06;
const NUDGE: f64 = 1e-4;

pub(crate) fn hue(index: usize) -> Color {
    let palette = palette();
    let hues = [
        palette.accent,
        palette.warning,
        palette.success,
        palette.danger,
        palette.accent.mix(palette.success, 0.5),
        palette.warning.mix(palette.danger, 0.5),
        palette.accent.mix(palette.danger, 0.5),
    ];
    hues[index % hues.len()]
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Curve {
    pub(crate) property: Property,
    pub(crate) index: usize,
    pub(crate) low: f64,
    pub(crate) high: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Plotting {
    pub(crate) plot: Plot,
    pub(crate) area: Rect,
    pub(crate) curves: Vec<Curve>,
}

pub(crate) fn sample(channel: &Channel<'_>, plot: Plot, local: f64) -> Option<f64> {
    match plot {
        Plot::Value => channel.graph_value(local),
        Plot::Speed => channel.speed(local),
    }
}

pub(crate) fn key_values(channel: &Channel<'_>, plot: Plot, local: i64) -> Vec<f64> {
    let at = local as f64;
    match plot {
        Plot::Value => channel.graph_value(at).into_iter().collect(),
        Plot::Speed => {
            let frames = channel.frames();
            let first = frames.first() == Some(&local);
            let last = frames.last() == Some(&local);
            let incoming = (!first).then(|| channel.speed(at - NUDGE)).flatten();
            let outgoing = (!last).then(|| channel.speed(at + NUDGE)).flatten();
            incoming.into_iter().chain(outgoing).collect()
        }
    }
}

pub(crate) fn plottable(clip: &Clip, property: &Property, plot: Plot) -> bool {
    match (plot, clip.channel(property)) {
        (_, None) => false,
        (Plot::Value, Some(channel)) => matches!(channel, Channel::Scalar(_)),
        (Plot::Speed, Some(_)) => true,
    }
}

pub(crate) fn leg(channel: &Channel<'_>, local: i64, side: Side) -> Option<i64> {
    let frames = channel.frames();
    let index = frames.iter().position(|frame| *frame == local)?;
    let other = match side {
        Side::Outgoing => frames.get(index + 1)?,
        Side::Incoming => frames.get(index.checked_sub(1)?)?,
    };
    Some((other - local).abs())
}

fn fit(clip: &Clip, property: &Property, plot: Plot) -> (f64, f64) {
    let Some(channel) = clip.channel(property) else {
        return (0.0, 1.0);
    };
    let last = (clip.length() - 1).max(0);
    let steps = last.clamp(1, SAMPLES);
    let mut values: Vec<f64> = (0..=steps)
        .filter_map(|step| sample(&channel, plot, (step * last) as f64 / steps as f64))
        .collect();
    for local in channel.frames() {
        values.extend(key_values(&channel, plot, local));
        if plot == Plot::Speed {
            values.extend(
                [Side::Incoming, Side::Outgoing]
                    .into_iter()
                    .filter_map(|side| channel.timing(local)?.handle(side))
                    .map(|handle| f64::from(handle.speed)),
            );
        }
    }
    match plot {
        Plot::Value => values.extend(channel.spans().iter().flat_map(|span| {
            [span.outgoing, span.incoming]
                .into_iter()
                .flatten()
                .map(|point| point[1])
        })),
        Plot::Speed => values.push(0.0),
    }
    let values: Vec<f64> = values
        .into_iter()
        .filter(|value| value.is_finite())
        .collect();
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !low.is_finite() || !high.is_finite() {
        return (0.0, 1.0);
    }
    let room = high - low;
    match room > 1e-9 {
        true => (low - room * MARGIN, high + room * MARGIN),
        false => {
            let around = (high.abs() * 0.1).max(1.0);
            (low - around, high + around)
        }
    }
}

impl Plotting {
    pub(crate) fn new(
        clip: &Clip,
        outline: &Outline,
        (plot, plotted): (Plot, &BTreeSet<Property>),
        area: Rect,
        frozen: Option<&Plotting>,
    ) -> Plotting {
        let curves = outline
            .properties
            .iter()
            .enumerate()
            .filter(|(_, property)| plotted.is_empty() || plotted.contains(*property))
            .filter(|(_, property)| plottable(clip, property, plot))
            .map(|(index, property)| {
                let range = frozen
                    .filter(|frozen| frozen.plot == plot)
                    .and_then(|frozen| frozen.curve(property));
                let (low, high) = match range {
                    Some(curve) => (curve.low, curve.high),
                    None => fit(clip, property, plot),
                };
                Curve {
                    property: property.clone(),
                    index,
                    low,
                    high,
                }
            })
            .collect();
        Plotting { plot, area, curves }
    }

    pub(crate) fn curve(&self, property: &Property) -> Option<&Curve> {
        self.curves.iter().find(|curve| curve.property == *property)
    }

    fn inner(&self) -> (f32, f32) {
        let top = self.area.y() + PAD;
        (top, (self.area.height() - PAD * 2.0).max(1.0))
    }

    pub(crate) fn y_of(&self, curve: &Curve, value: f64) -> f32 {
        let (top, height) = self.inner();
        let along = (value - curve.low) / (curve.high - curve.low);
        top + height * (1.0 - along as f32)
    }

    pub(crate) fn value_at(&self, curve: &Curve, y: f32) -> f64 {
        let (top, height) = self.inner();
        let along = f64::from(1.0 - (y - top) / height);
        curve.low + along * (curve.high - curve.low)
    }

    pub(crate) fn value_by(&self, curve: &Curve, rise: f32) -> f64 {
        let (_, height) = self.inner();
        -f64::from(rise / height) * (curve.high - curve.low)
    }
}

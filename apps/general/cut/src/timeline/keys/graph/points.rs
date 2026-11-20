use std::collections::BTreeSet;

use cut_project::{Clip, KeyRef, Property, Side};
use rikka_app::prelude::*;

use super::{Plotting, key_values, leg};
use crate::cut::Plot;
use crate::timeline::scale::Scale;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeyPoint {
    pub(crate) property: Property,
    pub(crate) index: usize,
    pub(crate) frame: i64,
    pub(crate) at: Point,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HandlePoint {
    pub(crate) property: Property,
    pub(crate) index: usize,
    pub(crate) frame: i64,
    pub(crate) side: Side,
    pub(crate) key: Point,
    pub(crate) tip: Point,
}

pub(crate) fn keys(plotting: &Plotting, clip: &Clip, scale: &Scale) -> Vec<KeyPoint> {
    let mut points = Vec::new();
    for curve in &plotting.curves {
        let Some(channel) = clip.channel(&curve.property) else {
            continue;
        };
        for local in channel.frames() {
            let frame = clip.start() + local;
            let x = scale.x_of(frame as f64);
            for value in key_values(&channel, plotting.plot, local) {
                points.push(KeyPoint {
                    property: curve.property.clone(),
                    index: curve.index,
                    frame,
                    at: Point::new(x, plotting.y_of(curve, value)),
                });
            }
        }
    }
    points
}

pub(crate) fn handles(
    plotting: &Plotting,
    clip: &Clip,
    picked: &BTreeSet<KeyRef>,
    scale: &Scale,
) -> Vec<HandlePoint> {
    let mut points = Vec::new();
    let point = |frame: f64, y: f32| Point::new(scale.x_of(clip.start() as f64 + frame), y);
    for curve in &plotting.curves {
        let Some(channel) = clip.channel(&curve.property) else {
            continue;
        };
        let selected = |local: i64| {
            picked.contains(&KeyRef::new(curve.property.clone(), clip.start() + local))
        };
        let mut push = |local: i64, side: Side, key: Point, tip: Point| {
            points.push(HandlePoint {
                property: curve.property.clone(),
                index: curve.index,
                frame: clip.start() + local,
                side,
                key,
                tip,
            });
        };
        match plotting.plot {
            Plot::Value => {
                for span in channel.spans() {
                    let (from, to) = (span.from[0].round() as i64, span.to[0].round() as i64);
                    let key = |at: [f64; 2]| point(at[0], plotting.y_of(curve, at[1]));
                    if let Some(tip) = span.outgoing.filter(|_| selected(from)) {
                        push(from, Side::Outgoing, key(span.from), key(tip));
                    }
                    if let Some(tip) = span.incoming.filter(|_| selected(to)) {
                        push(to, Side::Incoming, key(span.to), key(tip));
                    }
                }
            }
            Plot::Speed => {
                for local in channel
                    .frames()
                    .into_iter()
                    .filter(|local| selected(*local))
                {
                    let Some(timing) = channel.timing(local) else {
                        continue;
                    };
                    for side in [Side::Incoming, Side::Outgoing] {
                        let (Some(handle), Some(frames)) =
                            (timing.handle(side), leg(&channel, local, side))
                        else {
                            continue;
                        };
                        let y = plotting.y_of(curve, f64::from(handle.speed));
                        let reach = f64::from(handle.influence.min(1.0)) * frames as f64;
                        let tip = match side {
                            Side::Outgoing => local as f64 + reach,
                            Side::Incoming => local as f64 - reach,
                        };
                        push(local, side, point(local as f64, y), point(tip, y));
                    }
                }
            }
        }
    }
    points
}

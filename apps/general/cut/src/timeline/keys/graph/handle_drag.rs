use std::collections::BTreeSet;

use cut_project::{Clip, ClipId, Edit, Handle, KeyChange, KeyRef, Property, Side, Smooth};
use rikka_app::prelude::*;

use super::points::{self, KeyPoint};
use super::{Plotting, leg};
use crate::cut::Plot;
use crate::timeline::scale::Scale;

const REACH: f32 = 7.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Key {
        property: Property,
        frame: i64,
    },
    Handle {
        property: Property,
        frame: i64,
        side: Side,
    },
}

pub(crate) fn hit(
    plotting: &Plotting,
    clip: &Clip,
    picked: &BTreeSet<KeyRef>,
    scale: &Scale,
    local: Point,
) -> Option<Target> {
    let near = |at: Point| at.distance(local);
    let handle = points::handles(plotting, clip, picked, scale)
        .into_iter()
        .map(|point| (near(point.tip), point))
        .filter(|(gap, _)| *gap <= REACH)
        .min_by(|left, right| left.0.total_cmp(&right.0));
    if let Some((_, point)) = handle {
        return Some(Target::Handle {
            property: point.property,
            frame: point.frame,
            side: point.side,
        });
    }
    nearest_key(points::keys(plotting, clip, scale), local).map(|point| Target::Key {
        property: point.property,
        frame: point.frame,
    })
}

fn nearest_key(points: Vec<KeyPoint>, local: Point) -> Option<KeyPoint> {
    points
        .into_iter()
        .map(|point| (point.at.distance(local), point))
        .filter(|(gap, _)| *gap <= REACH)
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, point)| point)
}

pub(crate) fn banded(plotting: &Plotting, clip: &Clip, scale: &Scale, band: Rect) -> Vec<KeyRef> {
    points::keys(plotting, clip, scale)
        .into_iter()
        .filter(|point| band.contains(point.at))
        .map(|point| KeyRef::new(point.property, point.frame))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HandleDrag {
    pub(crate) property: Property,
    pub(crate) frame: i64,
    pub(crate) side: Side,
    breaking: bool,
    smooth: Smooth,
    pub(crate) handle: Option<Handle>,
}

impl HandleDrag {
    pub(crate) fn begin(
        clip: &Clip,
        (property, frame, side): (Property, i64, Side),
        breaking: bool,
    ) -> HandleDrag {
        let smooth = clip
            .channel(&property)
            .and_then(|channel| channel.timing(frame - clip.start()))
            .map_or(Smooth::Manual, |timing| timing.smooth);
        HandleDrag {
            property,
            frame,
            side,
            breaking,
            smooth,
            handle: None,
        }
    }

    pub(crate) fn drag(&mut self, plotting: &Plotting, clip: &Clip, scale: &Scale, to: Point) {
        let (Some(channel), Some(curve)) =
            (clip.channel(&self.property), plotting.curve(&self.property))
        else {
            return;
        };
        let local = self.frame - clip.start();
        let value = plotting.value_at(curve, to.y);
        let frame = scale.frame_at(to.x);
        self.handle = match plotting.plot {
            Plot::Value => {
                channel.handle_toward(local, self.side, [frame - clip.start() as f64, value])
            }
            Plot::Speed => leg(&channel, local, self.side).map(|frames| {
                let reach = ((frame - self.frame as f64).abs() / frames as f64) as f32;
                Handle::new(value as f32, reach).checked()
            }),
        };
    }

    pub(crate) fn edits(&self, clip: ClipId) -> Vec<Edit> {
        let key = |change: KeyChange| Edit::Key {
            clip,
            property: self.property.clone(),
            change,
        };
        let broken = (self.breaking && self.smooth != Smooth::Manual).then(|| {
            key(KeyChange::Smooth {
                at: self.frame,
                smooth: Smooth::Manual,
            })
        });
        let handle_edit = self.handle.map(|handle| {
            key(KeyChange::Handle {
                at: self.frame,
                side: self.side,
                handle,
            })
        });
        broken.into_iter().chain(handle_edit).collect()
    }
}

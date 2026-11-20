use rikka_figure::Affine;
use rikka_types::{Point, Rect};

use super::homography::Homography;
use super::lens::{Lens, Plane};
use crate::animation_presets::{PresetTransform, PresetTransforms, letter_count};
use crate::camera::Camera;
use crate::clip::{Clip, ClipId};
use crate::project::Project;
use crate::visual::{Appearance, Visual};

pub(super) const NEAR: f64 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Viewpoint {
    Output,
    Overview { camera: ClipId, scale: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    stages: Vec<Homography>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub visual: Visual,
    pub projection: Projection,
    pub path: Projection,
    pub transform: PresetTransform,
    pub letters: Vec<PresetTransform>,
    pub depth: f64,
    pub cameras: Vec<ClipId>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CameraLens {
    layer: usize,
    id: ClipId,
    lens: Lens,
}

impl Projection {
    pub(super) fn stages(&self) -> &[Homography] {
        &self.stages
    }
    #[must_use]
    pub fn homography(&self) -> Homography {
        self.stages
            .iter()
            .fold(Homography::IDENTITY, |total, stage| total.then(*stage))
    }

    #[must_use]
    pub fn affine(&self) -> Option<Affine> {
        self.homography().affine()
    }

    #[must_use]
    pub fn map(&self, local: Point) -> Option<Point> {
        self.stages.iter().try_fold(local, |point, stage| {
            (stage.depth(point) > NEAR).then_some(())?;
            stage.apply(point)
        })
    }

    #[must_use]
    pub fn unmap(&self, screen: Point) -> Option<Point> {
        self.stages.iter().rev().try_fold(screen, |point, stage| {
            let back = stage.inverse()?.apply(point)?;
            (stage.depth(back) > NEAR).then_some(back)
        })
    }
}

impl Project {
    #[must_use]
    pub fn shot(&self, clip: &Clip, frame: i64, view: Viewpoint) -> Option<Shot> {
        let visual = self.visual(clip, frame)?;
        self.shot_as(clip, frame, visual, view, |_| None)
    }

    #[must_use]
    pub fn camera_view(&self, camera: ClipId, frame: i64, scale: f32) -> Option<[Point; 4]> {
        let (_, clip) = self.locate(camera)?;
        let format = self.format();
        let flat = Lens::new(&clip.camera_at(frame)?, format).project(&Plane::picture(format));
        let back = flat.inverse()?;
        let outward = Homography::about(format.center(), f64::from(scale));
        let corners = Rect::new(0.0, 0.0, format.width as f32, format.height as f32).corners();
        let mut quad = [Point::ZERO; 4];
        for (point, corner) in quad.iter_mut().zip(corners) {
            let back_projected = back.apply(corner)?;
            if flat.depth(back_projected) <= NEAR {
                return None;
            }
            *point = outward.apply(back_projected)?;
        }
        Some(quad)
    }

    pub(super) fn cameras_at<'a>(
        &self,
        frame: i64,
        view: Viewpoint,
        restyled: &impl Fn(ClipId) -> Option<&'a Appearance>,
    ) -> Vec<CameraLens> {
        let ceiling = match view {
            Viewpoint::Output => usize::MAX,
            Viewpoint::Overview { camera, .. } => {
                self.locate(camera).map_or(usize::MAX, |(layer, _)| layer)
            }
        };
        let format = self.format();
        self.layers()
            .iter()
            .enumerate()
            .take(ceiling)
            .filter(|(_, layer)| layer.is_shown())
            .filter_map(|(layer, layer_ref)| {
                let clip = layer_ref.clip_at(frame)?;
                let camera = match restyled(clip.id()) {
                    Some(Appearance::Camera(camera)) if clip.camera().is_some() => camera.checked(),
                    _ => clip.camera_at(frame)?,
                };
                Some(CameraLens {
                    layer,
                    id: clip.id(),
                    lens: Lens::new(&camera, format),
                })
            })
            .collect()
    }

    pub(super) fn shoot(
        &self,
        layer: usize,
        visual: Visual,
        transforms: PresetTransforms,
        cameras: &[CameraLens],
        view: Viewpoint,
    ) -> Shot {
        let format = self.format();
        let visual = visual.with_preset_transform(&transforms.whole);
        let placement = visual.placement();
        let plane = Plane::from_placement(&placement);
        let chain: Vec<&CameraLens> = cameras
            .iter()
            .filter(|camera| camera.layer > layer)
            .collect();
        let nearest = chain.first().map_or_else(
            || Lens::new(&Camera::default(), format),
            |camera| camera.lens,
        );
        let through = |plane: &Plane| {
            let mut stages = vec![nearest.project(plane)];
            stages.extend(
                chain
                    .iter()
                    .skip(1)
                    .map(|camera| camera.lens.project(&Plane::picture(format))),
            );
            if let Viewpoint::Overview { scale, .. } = view {
                stages.push(Homography::about(format.center(), f64::from(scale)));
            }
            Projection { stages }
        };
        Shot {
            projection: through(&plane),
            path: through(&Plane::level(&placement)),
            visual,
            transform: transforms.whole,
            letters: transforms.letters,
            depth: nearest.depth(&plane),
            cameras: chain.iter().map(|camera| camera.id).collect(),
        }
    }
}

pub(super) fn preset_transforms(clip: &Clip, frame: i64, visual: &Visual) -> PresetTransforms {
    let letters = visual
        .graphic()
        .and_then(|graphic| graphic.art.lettering())
        .map_or(0, |lettering| letter_count(&lettering.text));
    clip.animation_presets()
        .transforms_at(clip.local(frame) as f64, clip.length(), letters)
}

pub(super) fn ordered<T>(entries: &mut [T], solid: impl Fn(&T) -> Option<&Shot>) {
    let mut start = 0;
    while start < entries.len() {
        let Some(first) = three_d(&entries[start], &solid) else {
            start += 1;
            continue;
        };
        let end = (start..entries.len())
            .find(|index| {
                three_d(&entries[*index], &solid).is_none_or(|shot| shot.cameras != first.cameras)
            })
            .unwrap_or(entries.len());
        entries[start..end].sort_by(|left, right| {
            let depth = |entry: &T| three_d(entry, &solid).map_or(0.0, |shot| shot.depth);
            depth(right).total_cmp(&depth(left))
        });
        start = end;
    }
}

fn three_d<'e, T>(entry: &'e T, solid: &impl Fn(&T) -> Option<&Shot>) -> Option<&'e Shot> {
    solid(entry).filter(|shot| shot.visual.placement().three_d)
}

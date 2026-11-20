use std::collections::BTreeMap;

use animate_body::{Body, HumanBone, Worlds};
use rikka_model_core::glam::Vec3;
use rikka_physics_tailor::Garment;

use crate::physique::{Flesh, Obstacle, SAMPLE, SHELLED, SLICE, strongest};
use crate::region::{Cover, Region};
use crate::shell::Shell;
use crate::surface::samples;

const LEAST_GIVE: f32 = 0.01;
const PRESSED_SHARE: f32 = 0.5;

struct Drape {
    point: Vec3,
    give: f32,
}

pub(crate) fn obstacles(
    body: &Body,
    rest: &Worlds,
    garments: &[Garment],
    folded: &[Option<HumanBone>],
    flesh: &[Flesh],
    scale: f32,
) -> Vec<Obstacle> {
    let mut drapes: BTreeMap<HumanBone, Vec<Drape>> = BTreeMap::new();
    for garment in garments {
        drape(body, garment, folded, scale, &mut drapes);
    }
    drapes
        .into_iter()
        .filter_map(|(bone, drapes)| covering(body, rest, bone, &drapes, flesh, scale))
        .collect()
}

fn drape(
    body: &Body,
    garment: &Garment,
    folded: &[Option<HumanBone>],
    scale: f32,
    drapes: &mut BTreeMap<HumanBone, Vec<Drape>>,
) {
    let anchors: Vec<Option<HumanBone>> = garment
        .vertices
        .iter()
        .map(|vertex| {
            strongest(&vertex.joints, &vertex.weights, folded)
                .and_then(|(bone, _)| anchor(body, bone))
        })
        .collect();
    for triangle in &garment.triangles {
        let corners = triangle.map(|corner| garment.vertices[corner as usize].rest);
        let gives = triangle.map(|corner| garment.vertices[corner as usize].give);
        for sample in samples(corners, SAMPLE * scale) {
            let Some(bone) = anchors[triangle[sample.nearest()] as usize] else {
                continue;
            };
            drapes.entry(bone).or_default().push(Drape {
                point: sample.point,
                give: sample.blend(gives),
            });
        }
    }
}

fn anchor(body: &Body, bone: HumanBone) -> Option<HumanBone> {
    let mut current = bone;
    loop {
        if Region::for_bone(current).is_arm() {
            return None;
        }
        if SHELLED.contains(&current) {
            return Some(current);
        }
        current = body.parent(current)?;
    }
}

fn covering(
    body: &Body,
    rest: &Worlds,
    bone: HumanBone,
    drapes: &[Drape],
    flesh: &[Flesh],
    scale: f32,
) -> Option<Obstacle> {
    let joint = rest.get(bone)?;
    let axis = body.direction(bone);
    let slice = SLICE * scale;
    let locals: Vec<Vec3> = drapes
        .iter()
        .map(|drape| joint.local(drape.point))
        .collect();
    let (low, high) = locals
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), local| {
            let height = local.dot(axis);
            (low.min(height), high.max(height))
        });
    let spanned = low - slice..=high + slice;
    let under: Vec<Vec3> = flesh
        .iter()
        .map(|flesh| joint.local(flesh.point))
        .filter(|local| spanned.contains(&local.dot(axis)))
        .collect();
    let skin = Shell::closed(axis, slice, &under);
    let least = LEAST_GIVE * scale;
    let insets: Vec<(Vec3, f32)> = drapes
        .iter()
        .zip(locals)
        .map(|(drape, local)| {
            let radius = local.reject_from_normalized(axis).length();
            let beneath = skin
                .as_ref()
                .and_then(|skin| skin.edge(local))
                .unwrap_or(0.0);
            let pressed = PRESSED_SHARE * (radius - beneath).max(0.0);
            (local, drape.give.max(pressed).max(least))
        })
        .collect();
    let shell = Shell::open(axis, slice, &insets)?;
    let (centre, radius) = shell.bound();
    Some(Obstacle {
        region: Region::for_bone(bone),
        cover: Cover::Garment,
        bone,
        shell,
        centre,
        radius,
    })
}

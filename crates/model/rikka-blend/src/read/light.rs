use std::f32::consts::PI;

use rikka_model_core::glam::Vec3;
use rikka_model_core::{Light, LightKind};

use super::material::nodes::Tree;
use super::modifier::Notes;
use crate::file::Record;

const POINT: i64 = 0;
const SUN: i64 = 1;
const SPOT: i64 = 2;
const HEMI: i64 = 3;
const AREA: i64 = 4;
const LUMENS_PER_WATT: f32 = 683.0;
const EMISSION: i64 = 140;

pub(crate) fn light(
    record: &Record<'_>,
    scene: Option<Record<'_>>,
    notes: &mut Notes,
) -> Option<Light> {
    let name = record.id_name();
    let kind = record.int("type").unwrap_or(POINT);
    let mut color = Vec3::new(
        record.float("r").unwrap_or(1.0),
        record.float("g").unwrap_or(1.0),
        record.float("b").unwrap_or(1.0),
    );
    let watts = match record.has("energy_new") {
        true => record.float("energy_new").unwrap_or(10.0),
        false => {
            let mut energy = record.float("energy").unwrap_or(10.0);
            let blend = record.blend();
            if !blend.at_least(280, 64) {
                let engine = scene
                    .and_then(|scene| scene.record("r"))
                    .and_then(|render| render.text("engine"))
                    .unwrap_or_else(|| "CYCLES".to_owned());
                match engine == "CYCLES" {
                    true => match emission(record) {
                        Some((strength, tint)) => {
                            energy = strength;
                            color = tint;
                        }
                        None => {
                            energy = match kind {
                                SUN => 1.0,
                                _ => 100.0,
                            }
                        }
                    },
                    false => {
                        if kind != SUN {
                            energy *= 100.0;
                        }
                    }
                }
            }
            energy
        }
    };
    let kind = match kind {
        SUN => LightKind::Directional,
        SPOT => {
            let outer = record.float("spotsize").unwrap_or(PI / 4.0) * 0.5;
            let blend = record.float("spotblend").unwrap_or(0.15);
            LightKind::Spot {
                range: f32::INFINITY,
                inner_angle: outer - outer * blend,
                outer_angle: outer,
            }
        }
        AREA => {
            notes.unsupported(format!(
                "skipping area light \"{name}\": no punctual equivalent"
            ));
            return None;
        }
        HEMI => {
            notes.unsupported(format!("hemi light \"{name}\" read as sun"));
            LightKind::Directional
        }
        _ => LightKind::Point {
            range: f32::INFINITY,
        },
    };
    let intensity = match kind {
        LightKind::Directional => watts * LUMENS_PER_WATT,
        _ => watts * LUMENS_PER_WATT / (4.0 * PI),
    };
    Some(Light::new(name, kind).color(color).intensity(intensity))
}

fn emission(record: &Record<'_>) -> Option<(f32, Vec3)> {
    if record.int("use_nodes").unwrap_or(0) == 0 {
        return None;
    }
    let tree = Tree::read(record.deref("nodetree")?);
    let (index, node) = tree
        .nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.is("ShaderNodeEmission", EMISSION))?;
    let strength = node.input(&["Strength"])?;
    let tint = node.input(&["Color"])?;
    if tree.linked(index, strength).is_some() || tree.linked(index, tint).is_some() {
        return None;
    }
    Some((
        node.inputs[strength].float()?,
        node.inputs[tint].color()?.truncate(),
    ))
}

use crate::animation::Track;
use crate::coordinates::{Conversion, CoordinateSystem};
use crate::error::Result;
use crate::mesh::Repeats;
use crate::model::Model;
use crate::options::{ImportOptions, Limit, Limits};

pub fn convert_coordinates(model: &mut Model, to: CoordinateSystem, scale: f32) {
    let conversion = Conversion::new(model.metadata.coordinates, to, scale);
    model.metadata.coordinates = to;
    if conversion.is_identity() {
        return;
    }

    let matrix = conversion.matrix();
    let inverse = matrix.inverse();
    let scale = conversion.scale();

    for mesh in &mut model.meshes {
        mesh.vertices.convert(&conversion);
        for target in &mut mesh.morph_targets {
            target.convert(&conversion);
        }
        if conversion.flips_winding() {
            mesh.flip_winding();
        }
        mesh.recompute_bounds();
    }
    for node in &mut model.nodes {
        node.transform = conversion.transform(node.transform);
    }
    for camera in &mut model.cameras {
        camera.projection = camera.projection.scaled(scale);
    }
    for light in &mut model.lights {
        light.kind = light.kind.scaled(scale);
    }
    for skin in &mut model.skins {
        for bind in &mut skin.inverse_bind {
            *bind = matrix * *bind * inverse;
        }
    }
    for animation in &mut model.animations {
        for channel in &mut animation.channels {
            match &mut channel.track {
                Track::Translation(curve) => {
                    for value in &mut curve.values {
                        *value = conversion.point(*value);
                    }
                }
                Track::Rotation(curve) => {
                    for value in &mut curve.values {
                        *value = conversion.rotation(*value);
                    }
                }
                Track::Scale(curve) => {
                    for value in &mut curve.values {
                        *value = conversion.scaling(*value);
                    }
                }
                Track::Weights(_) => {}
            }
        }
    }
    model.extensions.convert(&conversion);
}

pub(crate) fn finish(model: &mut Model, options: &ImportOptions) -> Result<()> {
    for mesh in &mut model.meshes {
        for target in &mut mesh.morph_targets {
            target.merge(Repeats::KeepFirst);
        }
    }
    check_limits(model, &options.limits)?;
    model.validate()?;

    let units = match options.normalize_units {
        true => model.metadata.unit_scale,
        false => 1.0,
    };
    convert_coordinates(model, options.coordinates, units * options.scale);
    if options.normalize_units {
        model.metadata.unit_scale = 1.0;
    }
    for mesh in &mut model.meshes {
        if options.normalize_weights {
            mesh.vertices.normalize_weights();
        }
        mesh.finalize();
    }
    Ok(())
}

fn check_limits(model: &Model, limits: &Limits) -> Result<()> {
    limits.check(Limit::Vertices, model.vertex_count() as u64)?;
    let indices: usize = model.meshes.iter().map(|mesh| mesh.indices.len()).sum();
    limits.check(Limit::Indices, indices as u64)?;
    let keyframes: usize = model
        .animations
        .iter()
        .flat_map(|animation| &animation.channels)
        .map(|channel| channel.track.len())
        .sum();
    limits.check(Limit::Keyframes, keyframes as u64)
}

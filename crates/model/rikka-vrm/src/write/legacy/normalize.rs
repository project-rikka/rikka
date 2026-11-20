use rikka_model_core::glam::{Mat3, Mat4, Vec3, Vec4};

use rikka_model_core::{Mesh, MeshId, Model, Transform};

const SHARED_TOLERANCE: f32 = 1e-6;

pub(super) struct Baked {
    pub corrections: Vec<Mat3>,
    pub changed: usize,
}

pub(super) fn bake(model: &mut Model) -> Baked {
    let world = model.world_transforms();
    let positions: Vec<Vec3> = world
        .iter()
        .map(|matrix| matrix.w_axis.truncate())
        .collect();
    let corrections: Vec<Mat3> = world
        .iter()
        .map(|matrix| Mat3::from_mat4(*matrix))
        .collect();
    let changed = model
        .nodes
        .iter()
        .filter(|node| {
            node.transform.rotation != Transform::IDENTITY.rotation
                || node.transform.scale != Transform::IDENTITY.scale
        })
        .count();

    for skin in &mut model.skins {
        for (joint, bind) in skin.joints.iter().zip(&mut skin.inverse_bind) {
            *bind =
                Mat4::from_translation(-positions[joint.index()]) * world[joint.index()] * *bind;
        }
    }
    bake_meshes(model, &corrections);

    for index in 0..model.nodes.len() {
        let parent = model.nodes[index]
            .parent
            .map_or(Vec3::ZERO, |parent| positions[parent.index()]);
        model.nodes[index].transform = Transform::from_translation(positions[index] - parent);
    }
    Baked {
        corrections,
        changed,
    }
}

fn bake_meshes(model: &mut Model, corrections: &[Mat3]) {
    let mut claims: Vec<Option<Mat3>> = vec![None; model.meshes.len()];
    let mut users = vec![0_usize; model.meshes.len()];
    for node in &model.nodes {
        let Some(mesh) = node.mesh else {
            continue;
        };
        users[mesh.index()] += 1;
        if node.skin.is_some() {
            claims[mesh.index()] = Some(Mat3::IDENTITY);
        }
    }
    let mut pristine: Vec<Option<Mesh>> = vec![None; model.meshes.len()];
    let mut clones: Vec<(MeshId, Mat3, MeshId)> = Vec::new();
    for index in 0..model.nodes.len() {
        let node = &model.nodes[index];
        let (Some(mesh), None) = (node.mesh, node.skin) else {
            continue;
        };
        let slot = mesh.index();
        let linear = corrections[index];
        match claims[slot] {
            None => {
                claims[slot] = Some(linear);
                if users[slot] > 1 {
                    pristine[slot] = Some(model.meshes[slot].clone());
                }
                transform_mesh(&mut model.meshes[slot], linear);
            }
            Some(claimed) if claimed.abs_diff_eq(linear, SHARED_TOLERANCE) => {}
            Some(_) => {
                let existing = clones.iter().find(|(original, claimed, _)| {
                    *original == mesh && claimed.abs_diff_eq(linear, SHARED_TOLERANCE)
                });
                let clone = match existing {
                    Some((_, _, clone)) => *clone,
                    None => {
                        let mut copy = pristine[slot]
                            .clone()
                            .unwrap_or_else(|| model.meshes[slot].clone());
                        transform_mesh(&mut copy, linear);
                        let clone = MeshId(model.meshes.len() as u32);
                        model.meshes.push(copy);
                        clones.push((mesh, linear, clone));
                        clone
                    }
                };
                model.nodes[index].mesh = Some(clone);
            }
        }
    }
}

fn transform_mesh(mesh: &mut Mesh, linear: Mat3) {
    if linear.abs_diff_eq(Mat3::IDENTITY, 0.0) {
        return;
    }
    let determinant = linear.determinant();
    let normal_matrix = cofactor(linear) * determinant.signum();
    let vertices = &mut mesh.vertices;
    for position in &mut vertices.positions {
        *position = linear * *position;
    }
    for normal in &mut vertices.normals {
        *normal = (normal_matrix * *normal).normalize_or(*normal);
    }
    for tangent in &mut vertices.tangents {
        let direction = tangent.truncate();
        let handedness = match determinant < 0.0 {
            true => -tangent.w,
            false => tangent.w,
        };
        *tangent = Vec4::from(((linear * direction).normalize_or(direction), handedness));
    }
    for target in &mut mesh.morph_targets {
        for position in &mut target.positions {
            *position = linear * *position;
        }
        for normal in &mut target.normals {
            *normal = normal_matrix * *normal;
        }
    }
    if determinant < 0.0 {
        mesh.flip_winding();
    }
    mesh.recompute_bounds();
}

fn cofactor(linear: Mat3) -> Mat3 {
    Mat3::from_cols(
        linear.y_axis.cross(linear.z_axis),
        linear.z_axis.cross(linear.x_axis),
        linear.x_axis.cross(linear.y_axis),
    )
}

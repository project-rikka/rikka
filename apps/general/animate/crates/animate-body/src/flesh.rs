use rikka_model_core::glam::{Mat4, Vec3};
use rikka_model_core::{HumanBone, Model};

pub(crate) fn flesh(
    model: &Model,
    folded: &[Option<HumanBone>],
    rest: &[Mat4],
    keeps: impl Fn(HumanBone) -> bool,
) -> Vec<(HumanBone, Vec3)> {
    let mut flesh = Vec::new();
    for (index, node) in model.nodes.iter().enumerate() {
        let Some(mesh) = node.mesh else {
            continue;
        };
        let vertices = &model.mesh(mesh).vertices;
        let Some(skin) = node.skin else {
            let Some(bone) = folded[index].filter(|bone| keeps(*bone)) else {
                continue;
            };
            flesh.extend(
                vertices
                    .positions
                    .iter()
                    .map(|position| (bone, rest[index].transform_point3(*position))),
            );
            continue;
        };
        let skin = model.skin(skin);
        let joints: Vec<Mat4> = skin
            .joints
            .iter()
            .zip(&skin.inverse_bind)
            .map(|(joint, bind)| rest[joint.index()] * *bind)
            .collect();
        for (vertex, position) in vertices.positions.iter().enumerate() {
            let (Some(slots), Some(weights)) =
                (vertices.joints.get(vertex), vertices.weights.get(vertex))
            else {
                continue;
            };
            let weights = weights.to_array();
            let Some(strongest) = slots
                .iter()
                .zip(weights)
                .filter(|(slot, _)| joints.get(usize::from(**slot)).is_some())
                .rev()
                .max_by(|one, other| one.1.total_cmp(&other.1))
                .map(|(slot, _)| usize::from(*slot))
            else {
                continue;
            };
            let Some(bone) = skin
                .joints
                .get(strongest)
                .and_then(|joint| folded[joint.index()])
                .filter(|bone| keeps(*bone))
            else {
                continue;
            };
            let mut placed = Vec3::ZERO;
            let mut total = 0.0;
            for (slot, weight) in slots.iter().zip(weights) {
                let Some(joint) = joints.get(usize::from(*slot)) else {
                    continue;
                };
                placed += joint.transform_point3(*position) * weight;
                total += weight;
            }
            if total <= 1e-6 {
                continue;
            }
            flesh.push((bone, placed / total));
        }
    }
    flesh
}

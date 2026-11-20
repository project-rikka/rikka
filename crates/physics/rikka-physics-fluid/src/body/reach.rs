use glam::{Mat4, Vec3};

use crate::body::mesh::BodyMesh;

#[derive(Clone, Copy, Debug)]
pub(crate) struct JointReach {
    centre: Vec3,
    radius: f32,
}

impl JointReach {
    pub(crate) fn compute(body: &BodyMesh) -> Vec<JointReach> {
        let count = body.palette as usize;
        let mut sums = vec![(Vec3::ZERO, 0u32); count];
        for (vertex, joints) in body.joints.iter().enumerate() {
            let position = Vec3::from_slice(&body.rest[vertex][..3]);
            for (slot, joint) in joints.iter().enumerate() {
                if body.weight_of(vertex, slot) > 0.0 {
                    let sum = &mut sums[*joint as usize];
                    sum.0 += position;
                    sum.1 += 1;
                }
            }
        }
        let mut reach: Vec<JointReach> = sums
            .iter()
            .map(|(sum, count)| JointReach {
                centre: match *count {
                    0 => Vec3::ZERO,
                    n => *sum / n as f32,
                },
                radius: 0.0,
            })
            .collect();
        for (vertex, joints) in body.joints.iter().enumerate() {
            let position = Vec3::from_slice(&body.rest[vertex][..3]);
            for (slot, joint) in joints.iter().enumerate() {
                if body.weight_of(vertex, slot) > 0.0 {
                    let joint_reach = &mut reach[*joint as usize];
                    joint_reach.radius = joint_reach
                        .radius
                        .max(position.distance(joint_reach.centre));
                }
            }
        }
        reach
    }

    pub(crate) fn travel(reach: &[JointReach], from: &[Mat4], to: &[Mat4]) -> f32 {
        reach
            .iter()
            .zip(from.iter().zip(to))
            .map(|(joint, (before, after))| {
                let change = *after - *before;
                let moved = change.transform_point3(joint.centre).length();
                let turned = (change.x_axis.truncate().length_squared()
                    + change.y_axis.truncate().length_squared()
                    + change.z_axis.truncate().length_squared())
                .sqrt();
                moved + turned * joint.radius
            })
            .fold(0.0, f32::max)
    }
}

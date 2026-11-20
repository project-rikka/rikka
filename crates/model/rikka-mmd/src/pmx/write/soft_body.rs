use rikka_model_core::ByteWriter;

use crate::pmx::document::PmxIndexSizes;
use crate::pmx::soft_body::{PmxSoftBody, PmxSoftBodyConfig};
use crate::text::PmxEncoding;

use super::wire::{index, vertex_index};

pub(super) fn write_soft_bodies(
    writer: &mut ByteWriter,
    soft_bodies: &[PmxSoftBody],
    encoding: PmxEncoding,
    sizes: &PmxIndexSizes,
) {
    writer.i32(soft_bodies.len() as i32);
    for soft_body in soft_bodies {
        encoding.write(writer, &soft_body.name);
        encoding.write(writer, &soft_body.english_name);
        writer.u8(soft_body.shape.byte());
        index(writer, sizes.material, soft_body.material);
        writer.u8(soft_body.group);
        writer.u16(soft_body.collision_mask);
        writer.u8(soft_body.flags.0);
        writer.i32(soft_body.bending_distance);
        writer.i32(soft_body.cluster_count);
        writer.f32(soft_body.total_mass);
        writer.f32(soft_body.collision_margin);
        writer.i32(soft_body.aero_model.code());
        write_config(writer, &soft_body.config);
        let cluster = &soft_body.cluster;
        writer.f32s(&[
            cluster.soft_rigid_hardness,
            cluster.soft_kinetic_hardness,
            cluster.soft_soft_hardness,
            cluster.soft_rigid_impulse_split,
            cluster.soft_kinetic_impulse_split,
            cluster.soft_soft_impulse_split,
        ]);
        let iterations = &soft_body.iterations;
        for count in [
            iterations.velocity,
            iterations.position,
            iterations.drift,
            iterations.cluster,
        ] {
            writer.i32(count);
        }
        let stiffness = &soft_body.material_stiffness;
        writer.f32s(&[
            stiffness.linear_stiffness,
            stiffness.area_stiffness,
            stiffness.volume_stiffness,
        ]);
        writer.i32(soft_body.anchors.len() as i32);
        for anchor in &soft_body.anchors {
            index(writer, sizes.rigid_body, Some(anchor.body));
            vertex_index(writer, sizes.vertex, anchor.vertex);
            writer.u8(u8::from(anchor.near_mode));
        }
        writer.i32(soft_body.pins.len() as i32);
        for pin in &soft_body.pins {
            vertex_index(writer, sizes.vertex, *pin);
        }
    }
}

fn write_config(writer: &mut ByteWriter, config: &PmxSoftBodyConfig) {
    writer.f32s(&[
        config.velocity_correction,
        config.damping,
        config.drag,
        config.lift,
        config.pressure,
        config.volume_conservation,
        config.dynamic_friction,
        config.pose_matching,
        config.rigid_contact_hardness,
        config.kinetic_contact_hardness,
        config.soft_contact_hardness,
        config.anchor_hardness,
    ]);
}

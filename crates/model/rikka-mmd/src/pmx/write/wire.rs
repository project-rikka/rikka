use rikka_model_core::glam::Vec4;
use rikka_model_core::{ByteWriter, ExportCtx, INFLUENCES, Result};
use rustc_hash::FxHashMap;

use crate::euler;
use crate::pmx::document::{
    Pmx, PmxBlend, PmxBone, PmxBoneFlags, PmxDisplayFrame, PmxDisplayTarget, PmxIndexSizes,
    PmxJoint, PmxLimits, PmxMaterial, PmxMorph, PmxOffsets, PmxRigidBody, PmxSpherical, PmxTail,
    PmxToon, PmxWeight,
};
use crate::text::PmxEncoding;

use super::build::{Morph, PmxGeometry};

pub(super) fn index(writer: &mut ByteWriter, size: u8, index: Option<usize>) {
    let value = index
        .and_then(|index| i32::try_from(index).ok())
        .unwrap_or(-1);
    match size {
        1 => writer.u8(value as i8 as u8),
        2 => writer.u16(value as i16 as u16),
        _ => writer.i32(value),
    }
}

pub(super) fn vertex_index(writer: &mut ByteWriter, size: u8, index: u32) {
    match size {
        1 => writer.u8(index as u8),
        2 => writer.u16(index as u16),
        _ => writer.u32(index),
    }
}

pub(super) fn write_vertices(
    writer: &mut ByteWriter,
    geometry: &PmxGeometry,
    pmx: Option<&Pmx>,
    sizes: &PmxIndexSizes,
    ctx: &mut ExportCtx<'_>,
) -> Result<()> {
    let spherical: FxHashMap<u32, &PmxSpherical> = pmx
        .map(|pmx| {
            pmx.vertices
                .spherical
                .iter()
                .map(|sdef| (sdef.vertex, sdef))
                .collect()
        })
        .unwrap_or_default();
    let kinds = pmx
        .map(|pmx| pmx.vertices.weights.as_slice())
        .unwrap_or(&[]);
    let edges = pmx
        .map(|pmx| pmx.vertices.edge_scales.as_slice())
        .unwrap_or(&[]);
    let extra = pmx
        .map(|pmx| pmx.vertices.additional_uvs.as_slice())
        .unwrap_or(&[]);

    writer.i32(geometry.positions.len() as i32);
    for vertex in 0..geometry.positions.len() {
        if vertex % 4096 == 0 {
            ctx.checkpoint()?;
        }
        writer.vec3(geometry.positions[vertex]);
        writer.vec3(geometry.normals[vertex]);
        writer.vec2(geometry.uvs[vertex]);
        for uvs in extra {
            writer.vec4(uvs.get(vertex).copied().unwrap_or(Vec4::ZERO));
        }

        let binding = &geometry.bindings[vertex];
        let bound = binding.iter().filter(|(_, weight)| *weight > 0.0).count();
        let sdef = spherical.get(&(vertex as u32));
        let kind = match kinds.get(vertex) {
            Some(PmxWeight::Spherical) if sdef.is_some() => PmxWeight::Spherical,
            Some(PmxWeight::Quaternion) => PmxWeight::Quaternion,
            _ => match bound {
                0 | 1 => PmxWeight::One,
                2 => PmxWeight::Two,
                _ => PmxWeight::Four,
            },
        };
        let bone = |writer: &mut ByteWriter, slot: usize| {
            index(writer, sizes.bone, Some(usize::from(binding[slot].0)));
        };
        match kind {
            PmxWeight::One => {
                writer.u8(0);
                bone(writer, 0);
            }
            PmxWeight::Two | PmxWeight::Spherical => {
                writer.u8(match kind {
                    PmxWeight::Two => 1,
                    _ => 3,
                });
                bone(writer, 0);
                bone(writer, 1);
                let total = binding[0].1 + binding[1].1;
                writer.f32(match total > 0.0 {
                    true => binding[0].1 / total,
                    false => 1.0,
                });
                if let (PmxWeight::Spherical, Some(sdef)) = (kind, sdef) {
                    writer.vec3(sdef.centre);
                    writer.vec3(sdef.radius0);
                    writer.vec3(sdef.radius1);
                }
            }
            PmxWeight::Four | PmxWeight::Quaternion => {
                writer.u8(match kind {
                    PmxWeight::Four => 2,
                    _ => 4,
                });
                for slot in 0..INFLUENCES {
                    bone(writer, slot);
                }
                for (_, weight) in binding {
                    writer.f32(*weight);
                }
            }
        }
        writer.f32(edges.get(vertex).copied().unwrap_or(1.0));
    }
    Ok(())
}

pub(super) fn write_faces(writer: &mut ByteWriter, geometry: &PmxGeometry, sizes: &PmxIndexSizes) {
    writer.i32(geometry.indices.len() as i32);
    for vertex in &geometry.indices {
        vertex_index(writer, sizes.vertex, *vertex);
    }
}

pub(super) fn write_materials(
    writer: &mut ByteWriter,
    materials: &[PmxMaterial],
    geometry: &PmxGeometry,
    encoding: PmxEncoding,
    sizes: &PmxIndexSizes,
) {
    writer.i32(materials.len() as i32);
    for (material, run) in materials.iter().zip(&geometry.runs) {
        encoding.write(writer, &material.name);
        encoding.write(writer, &material.english_name);
        writer.vec4(material.diffuse);
        writer.vec3(material.specular);
        writer.f32(material.specular_power);
        writer.vec3(material.ambient);
        writer.u8(material.flags.0);
        writer.vec4(material.edge_color);
        writer.f32(material.edge_size);
        index(writer, sizes.texture, material.texture);
        index(writer, sizes.texture, material.sphere);
        writer.u8(material.sphere_mode.byte());
        match material.toon {
            PmxToon::Shared(toon) => {
                writer.u8(1);
                writer.u8(toon);
            }
            PmxToon::Texture(texture) => {
                writer.u8(0);
                index(writer, sizes.texture, texture);
            }
        }
        encoding.write(writer, &material.memo);
        writer.i32(run.indices as i32);
    }
}

pub(super) fn write_bones(
    writer: &mut ByteWriter,
    bones: &[PmxBone],
    encoding: PmxEncoding,
    sizes: &PmxIndexSizes,
) {
    writer.i32(bones.len() as i32);
    for bone in bones {
        encoding.write(writer, &bone.name);
        encoding.write(writer, &bone.english_name);
        writer.vec3(bone.position);
        index(writer, sizes.bone, bone.parent);
        writer.i32(bone.layer);

        let mut flags = bone.flags;
        flags.set(
            PmxBoneFlags::TAIL_IS_BONE,
            matches!(bone.tail, PmxTail::Bone(_)),
        );
        flags.set(
            PmxBoneFlags::APPEND_ROTATE,
            bone.append.is_some() && flags.has(PmxBoneFlags::APPEND_ROTATE),
        );
        flags.set(
            PmxBoneFlags::APPEND_TRANSLATE,
            bone.append.is_some() && flags.has(PmxBoneFlags::APPEND_TRANSLATE),
        );
        flags.set(PmxBoneFlags::FIXED_AXIS, bone.fixed_axis.is_some());
        flags.set(PmxBoneFlags::LOCAL_AXES, bone.local_axes.is_some());
        flags.set(
            PmxBoneFlags::EXTERNAL_PARENT,
            bone.external_parent.is_some(),
        );
        flags.set(PmxBoneFlags::IK, bone.ik.is_some());
        writer.u16(flags.0);

        match bone.tail {
            PmxTail::Bone(tail) => index(writer, sizes.bone, tail),
            PmxTail::Offset(offset) => writer.vec3(offset),
        }
        if let Some(append) = bone
            .append
            .filter(|_| flags.has(PmxBoneFlags::APPEND_ROTATE | PmxBoneFlags::APPEND_TRANSLATE))
        {
            index(writer, sizes.bone, Some(append.parent));
            writer.f32(append.ratio);
        }
        if let Some(axis) = bone.fixed_axis {
            writer.vec3(axis);
        }
        if let Some((x, z)) = bone.local_axes {
            writer.vec3(x);
            writer.vec3(z);
        }
        if let Some(key) = bone.external_parent {
            writer.i32(key);
        }
        if let Some(ik) = &bone.ik {
            index(writer, sizes.bone, Some(ik.target));
            writer.i32(ik.loops as i32);
            writer.f32(ik.limit_angle);
            writer.i32(ik.links.len() as i32);
            for link in &ik.links {
                index(writer, sizes.bone, Some(link.bone));
                match link.limits {
                    Some(PmxLimits { lower, upper }) => {
                        writer.u8(1);
                        writer.vec3(lower);
                        writer.vec3(upper);
                    }
                    None => writer.u8(0),
                }
            }
        }
    }
}

pub(super) fn write_morphs(
    writer: &mut ByteWriter,
    morphs: &[PmxMorph],
    geometry: &PmxGeometry,
    encoding: PmxEncoding,
    sizes: &PmxIndexSizes,
) {
    writer.i32(morphs.len() as i32);
    for (index_of, morph) in morphs.iter().enumerate() {
        encoding.write(writer, &morph.name);
        encoding.write(writer, &morph.english_name);
        writer.u8(morph.panel.byte());
        match &morph.offsets {
            PmxOffsets::Group(offsets) => {
                writer.u8(0);
                writer.i32(offsets.len() as i32);
                for offset in offsets {
                    index(writer, sizes.morph, Some(offset.morph));
                    writer.f32(offset.ratio);
                }
            }
            PmxOffsets::Vertex => {
                let empty = Morph {
                    name: String::new(),
                    vertices: Vec::new(),
                    offsets: Vec::new(),
                };
                let morph = geometry.morphs.get(index_of).unwrap_or(&empty);
                writer.u8(1);
                writer.i32(morph.vertices.len() as i32);
                for (vertex, offset) in morph.vertices.iter().zip(&morph.offsets) {
                    vertex_index(writer, sizes.vertex, *vertex);
                    writer.vec3(*offset);
                }
            }
            PmxOffsets::Bone(offsets) => {
                writer.u8(2);
                writer.i32(offsets.len() as i32);
                for offset in offsets {
                    index(writer, sizes.bone, Some(offset.bone));
                    writer.vec3(offset.translation);
                    writer.vec4(Vec4::from(offset.rotation));
                }
            }
            PmxOffsets::Uv { channel, offsets } => {
                writer.u8(3 + channel);
                writer.i32(offsets.len() as i32);
                for offset in offsets {
                    vertex_index(writer, sizes.vertex, offset.vertex);
                    writer.vec4(offset.offset);
                }
            }
            PmxOffsets::Material(offsets) => {
                writer.u8(8);
                writer.i32(offsets.len() as i32);
                for offset in offsets {
                    index(writer, sizes.material, offset.material);
                    writer.u8(match offset.blend {
                        PmxBlend::Multiply => 0,
                        PmxBlend::Add => 1,
                    });
                    writer.vec4(offset.diffuse);
                    writer.vec3(offset.specular);
                    writer.f32(offset.specular_power);
                    writer.vec3(offset.ambient);
                    writer.vec4(offset.edge_color);
                    writer.f32(offset.edge_size);
                    writer.vec4(offset.texture_tint);
                    writer.vec4(offset.sphere_tint);
                    writer.vec4(offset.toon_tint);
                }
            }
            PmxOffsets::Flip(offsets) => {
                writer.u8(9);
                writer.i32(offsets.len() as i32);
                for offset in offsets {
                    index(writer, sizes.morph, Some(offset.morph));
                    writer.f32(offset.ratio);
                }
            }
            PmxOffsets::Impulse(offsets) => {
                writer.u8(10);
                writer.i32(offsets.len() as i32);
                for offset in offsets {
                    index(writer, sizes.rigid_body, Some(offset.body));
                    writer.u8(u8::from(offset.local));
                    writer.vec3(offset.velocity);
                    writer.vec3(offset.torque);
                }
            }
        }
    }
}

pub(super) fn write_frames(
    writer: &mut ByteWriter,
    frames: &[PmxDisplayFrame],
    encoding: PmxEncoding,
    sizes: &PmxIndexSizes,
) {
    writer.i32(frames.len() as i32);
    for frame in frames {
        encoding.write(writer, &frame.name);
        encoding.write(writer, &frame.english_name);
        writer.u8(u8::from(frame.special));
        writer.i32(frame.targets.len() as i32);
        for target in &frame.targets {
            match target {
                PmxDisplayTarget::Bone(bone) => {
                    writer.u8(0);
                    index(writer, sizes.bone, Some(*bone));
                }
                PmxDisplayTarget::Morph(morph) => {
                    writer.u8(1);
                    index(writer, sizes.morph, Some(*morph));
                }
            }
        }
    }
}

pub(super) fn write_rigid_bodies(
    writer: &mut ByteWriter,
    bodies: &[PmxRigidBody],
    encoding: PmxEncoding,
    sizes: &PmxIndexSizes,
) {
    writer.i32(bodies.len() as i32);
    for body in bodies {
        encoding.write(writer, &body.name);
        encoding.write(writer, &body.english_name);
        index(writer, sizes.bone, body.bone);
        writer.u8(body.group);
        writer.u16(body.collision_mask);
        writer.u8(body.shape.byte());
        writer.vec3(body.size);
        writer.vec3(body.position);
        writer.vec3(euler::angles_of(body.rotation));
        writer.f32(body.mass);
        writer.f32(body.linear_damping);
        writer.f32(body.angular_damping);
        writer.f32(body.restitution);
        writer.f32(body.friction);
        writer.u8(body.physics.byte());
    }
}

pub(super) fn write_joints(
    writer: &mut ByteWriter,
    joints: &[PmxJoint],
    encoding: PmxEncoding,
    sizes: &PmxIndexSizes,
) {
    writer.i32(joints.len() as i32);
    for joint in joints {
        encoding.write(writer, &joint.name);
        encoding.write(writer, &joint.english_name);
        writer.u8(joint.kind);
        index(writer, sizes.rigid_body, joint.bodies.0);
        index(writer, sizes.rigid_body, joint.bodies.1);
        writer.vec3(joint.position);
        writer.vec3(euler::angles_of(joint.rotation));
        writer.vec3(joint.translation_limits.lower);
        writer.vec3(joint.translation_limits.upper);
        writer.vec3(joint.rotation_limits.lower);
        writer.vec3(joint.rotation_limits.upper);
        writer.vec3(joint.translation_spring);
        writer.vec3(joint.rotation_spring);
    }
}

use rikka_model_core::{ByteReader, Error, Result};

use crate::pmx::soft_body::{
    PmxAeroModel, PmxAnchor, PmxClusterConfig, PmxSoftBody, PmxSoftBodyConfig, PmxSoftBodyFlags,
    PmxSoftBodyIterations, PmxSoftBodyMaterial, PmxSoftBodyShape,
};

use super::wire::{Header, index, named, vertex_index};

const SOFT_BODY: usize = 142;
const ANCHOR: usize = 3;

pub(super) fn read_soft_bodies(
    reader: &mut ByteReader<'_>,
    header: &Header,
    vertices: usize,
    materials: usize,
    rigid_bodies: usize,
) -> Result<Vec<PmxSoftBody>> {
    let count = reader.count(SOFT_BODY, "soft bodies")?;
    let mut soft_bodies = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, english_name) = named(reader, header)?;
        let shape = reader.u8()?;
        let shape = PmxSoftBodyShape::from_byte(shape).ok_or_else(|| {
            Error::Damaged(format!("soft body \"{name}\" has unknown shape {shape}"))
        })?;
        let material = index(reader, header.sizes.material)?;
        if let Some(material) = material.filter(|material| *material >= materials) {
            return Err(Error::Damaged(format!(
                "soft body \"{name}\" names material {material} of {materials}"
            )));
        }
        let group = reader.u8()?;
        let collision_mask = reader.u16()?;
        let flags = PmxSoftBodyFlags(reader.u8()?);
        let bending_distance = reader.i32()?;
        let cluster_count = reader.i32()?;
        let total_mass = reader.f32()?;
        let collision_margin = reader.f32()?;
        let aero_model = reader.i32()?;
        let aero_model = PmxAeroModel::from_code(aero_model).ok_or_else(|| {
            Error::Damaged(format!(
                "soft body \"{name}\" has unknown aerodynamic model {aero_model}"
            ))
        })?;
        let config = read_config(reader)?;
        let cluster = PmxClusterConfig {
            soft_rigid_hardness: reader.f32()?,
            soft_kinetic_hardness: reader.f32()?,
            soft_soft_hardness: reader.f32()?,
            soft_rigid_impulse_split: reader.f32()?,
            soft_kinetic_impulse_split: reader.f32()?,
            soft_soft_impulse_split: reader.f32()?,
        };
        let iterations = PmxSoftBodyIterations {
            velocity: reader.i32()?,
            position: reader.i32()?,
            drift: reader.i32()?,
            cluster: reader.i32()?,
        };
        let material_stiffness = PmxSoftBodyMaterial {
            linear_stiffness: reader.f32()?,
            area_stiffness: reader.f32()?,
            volume_stiffness: reader.f32()?,
        };
        let anchors = read_anchors(reader, header, &name, vertices, rigid_bodies)?;
        let pins = read_pins(reader, header, &name, vertices)?;
        soft_bodies.push(PmxSoftBody {
            name,
            english_name,
            shape,
            material,
            group,
            collision_mask,
            flags,
            bending_distance,
            cluster_count,
            total_mass,
            collision_margin,
            aero_model,
            config,
            cluster,
            iterations,
            material_stiffness,
            anchors,
            pins,
        });
    }
    Ok(soft_bodies)
}

fn read_config(reader: &mut ByteReader<'_>) -> Result<PmxSoftBodyConfig> {
    Ok(PmxSoftBodyConfig {
        velocity_correction: reader.f32()?,
        damping: reader.f32()?,
        drag: reader.f32()?,
        lift: reader.f32()?,
        pressure: reader.f32()?,
        volume_conservation: reader.f32()?,
        dynamic_friction: reader.f32()?,
        pose_matching: reader.f32()?,
        rigid_contact_hardness: reader.f32()?,
        kinetic_contact_hardness: reader.f32()?,
        soft_contact_hardness: reader.f32()?,
        anchor_hardness: reader.f32()?,
    })
}

fn read_anchors(
    reader: &mut ByteReader<'_>,
    header: &Header,
    name: &str,
    vertices: usize,
    rigid_bodies: usize,
) -> Result<Vec<PmxAnchor>> {
    let count = reader.count(ANCHOR, "soft body anchors")?;
    let mut anchors = Vec::with_capacity(count);
    for _ in 0..count {
        let body = index(reader, header.sizes.rigid_body)?;
        let Some(body) = body.filter(|body| *body < rigid_bodies) else {
            return Err(Error::Damaged(format!(
                "soft body \"{name}\" anchors to a rigid body outside {rigid_bodies}"
            )));
        };
        let vertex = vertex_index(reader, header.sizes.vertex)?;
        if vertex as usize >= vertices {
            return Err(Error::Damaged(format!(
                "soft body \"{name}\" anchors vertex {vertex} of {vertices}"
            )));
        }
        let near_mode = reader.u8()? != 0;
        anchors.push(PmxAnchor {
            body,
            vertex,
            near_mode,
        });
    }
    Ok(anchors)
}

fn read_pins(
    reader: &mut ByteReader<'_>,
    header: &Header,
    name: &str,
    vertices: usize,
) -> Result<Vec<u32>> {
    let count = reader.count(usize::from(header.sizes.vertex), "soft body pins")?;
    let mut pins = Vec::with_capacity(count);
    for _ in 0..count {
        let vertex = vertex_index(reader, header.sizes.vertex)?;
        if vertex as usize >= vertices {
            return Err(Error::Damaged(format!(
                "soft body \"{name}\" pins vertex {vertex} of {vertices}"
            )));
        }
        pins.push(vertex);
    }
    Ok(pins)
}

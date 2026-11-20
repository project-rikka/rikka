use rikka_model_core::glam::Vec3;
use rikka_model_core::{
    ByteReader, Error, INFLUENCES, ImportCtx, MorphTarget, NodeId, Repeats, Result, Stage, Vertices,
};

use crate::pmx::document::{
    Pmx, PmxAppend, PmxBlend, PmxBone, PmxBoneFlags, PmxBoneOffset, PmxDisplayFrame,
    PmxDisplayTarget, PmxGroupOffset, PmxIk, PmxIkLink, PmxImpulseOffset, PmxIndexSizes, PmxJoint,
    PmxJointKind, PmxLimits, PmxMaterial, PmxMaterialFlags, PmxMaterialOffset, PmxMorph,
    PmxOffsets, PmxPanel, PmxPhysics, PmxRigidBody, PmxShape, PmxSphereMode, PmxSpherical, PmxTail,
    PmxToon, PmxUvOffset, PmxVertices, PmxWeight,
};
use crate::pmx::version::PmxVersion;
use crate::text::PmxEncoding;
use crate::{MAGIC, euler};

use super::soft_body::read_soft_bodies;

const MATERIAL: usize = 86;
const BONE: usize = 27;
const IK_LINK: usize = 2;
const MORPH: usize = 14;
const MORPH_OFFSET: usize = 5;
const DISPLAY_FRAME: usize = 13;
const DISPLAY_TARGET: usize = 2;
const RIGID_BODY: usize = 70;
const JOINT: usize = 107;

pub(super) struct PmxFile {
    pub(super) pmx: Pmx,
    pub(super) vertices: Vertices,
    pub(super) indices: Vec<u32>,
    pub(super) counts: Vec<u32>,
    pub(super) targets: Vec<MorphTarget>,
}

pub(super) fn parse(bytes: &[u8], ctx: &mut ImportCtx<'_>) -> Result<PmxFile> {
    let mut reader = ByteReader::new(bytes);
    let header = Header::read(&mut reader)?;
    let mut pmx = Pmx {
        encoding: header.encoding,
        index_sizes: header.sizes,
        ..Pmx::default()
    };
    pmx.name = header.encoding.read(&mut reader)?;
    pmx.english_name = header.encoding.read(&mut reader)?;
    pmx.comment = header.encoding.read(&mut reader)?;
    pmx.english_comment = header.encoding.read(&mut reader)?;

    let (vertices, mut extra) = read_vertices(&mut reader, &header, ctx)?;
    ctx.report(Stage::Parsing, 0.3);
    let indices = read_faces(&mut reader, &header, vertices.len(), ctx)?;
    ctx.report(Stage::Parsing, 0.5);
    pmx.textures = read_textures(&mut reader, &header)?;
    let (materials, counts) = read_materials(&mut reader, &header, &pmx.textures)?;
    pmx.materials = materials;
    pmx.bones = read_bones(&mut reader, &header)?;
    ctx.report(Stage::Parsing, 0.7);
    let (morphs, targets) = read_morphs(&mut reader, &header, &vertices)?;
    pmx.morphs = morphs;
    pmx.display_frames = read_display_frames(&mut reader, &header)?;
    pmx.rigid_bodies = read_rigid_bodies(&mut reader, &header)?;
    pmx.joints = read_joints(&mut reader, &header)?;
    if header.version == PmxVersion::V2_1 {
        pmx.soft_bodies = read_soft_bodies(
            &mut reader,
            &header,
            vertices.len(),
            pmx.materials.len(),
            pmx.rigid_bodies.len(),
        )?;
    }
    drop_dangling_references(&mut pmx);
    ctx.report(Stage::Parsing, 0.9);

    extra.edge_scales.truncate(vertices.len());
    pmx.vertices = extra;
    Ok(PmxFile {
        pmx,
        vertices,
        indices,
        counts,
        targets,
    })
}

pub(super) struct Header {
    pub(super) version: PmxVersion,
    pub(super) encoding: PmxEncoding,
    additional_uvs: u8,
    pub(super) sizes: PmxIndexSizes,
}

impl Header {
    fn read(reader: &mut ByteReader<'_>) -> Result<Self> {
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(Error::Damaged("missing PMX magic".to_owned()));
        }
        let number = reader.f32()?;
        let Some(version) = PmxVersion::from_number(number) else {
            return Err(Error::UnsupportedVersion {
                format: "PMX",
                version: format!("{number:.1}"),
            });
        };
        let globals = usize::from(reader.u8()?);
        if globals < 8 {
            return Err(Error::Damaged(format!(
                "PMX header has {globals} globals, expected 8"
            )));
        }
        let globals_bytes = reader.take(globals)?;
        let width = |value: u8, what: &str| match value {
            1 | 2 | 4 => Ok(value),
            other => Err(Error::Damaged(format!("invalid {what} index size {other}"))),
        };
        if globals_bytes[1] > 4 {
            return Err(Error::Damaged(format!(
                "PMX declares {} additional UV sets, at most 4 are allowed",
                globals_bytes[1]
            )));
        }
        Ok(Self {
            version,
            encoding: PmxEncoding::from_byte(globals_bytes[0])?,
            additional_uvs: globals_bytes[1],
            sizes: PmxIndexSizes {
                vertex: width(globals_bytes[2], "vertex")?,
                texture: width(globals_bytes[3], "texture")?,
                material: width(globals_bytes[4], "material")?,
                bone: width(globals_bytes[5], "bone")?,
                morph: width(globals_bytes[6], "morph")?,
                rigid_body: width(globals_bytes[7], "rigid body")?,
            },
        })
    }
}

pub(super) fn index(reader: &mut ByteReader<'_>, size: u8) -> Result<Option<usize>> {
    let raw = match size {
        1 => i32::from(reader.u8()? as i8),
        2 => i32::from(reader.u16()? as i16),
        _ => reader.i32()?,
    };
    Ok(usize::try_from(raw).ok())
}

pub(super) fn vertex_index(reader: &mut ByteReader<'_>, size: u8) -> Result<u32> {
    match size {
        1 => Ok(u32::from(reader.u8()?)),
        2 => Ok(u32::from(reader.u16()?)),
        _ => Ok(reader.u32()?),
    }
}

pub(super) fn named(reader: &mut ByteReader<'_>, header: &Header) -> Result<(String, String)> {
    Ok((header.encoding.read(reader)?, header.encoding.read(reader)?))
}

fn read_vertices(
    reader: &mut ByteReader<'_>,
    header: &Header,
    ctx: &mut ImportCtx<'_>,
) -> Result<(Vertices, PmxVertices)> {
    let stride = 8 * 4 + usize::from(header.additional_uvs) * 16 + 1 + 4;
    let count = reader.count(stride, "vertices")?;
    let mut vertices = Vertices {
        positions: Vec::with_capacity(count),
        normals: Vec::with_capacity(count),
        uvs: Vec::with_capacity(count),
        joints: Vec::with_capacity(count),
        weights: Vec::with_capacity(count),
        ..Vertices::default()
    };
    let mut extra = PmxVertices {
        weights: Vec::with_capacity(count),
        additional_uvs: vec![Vec::with_capacity(count); usize::from(header.additional_uvs)],
        spherical: Vec::new(),
        edge_scales: Vec::with_capacity(count),
    };

    for vertex in 0..count {
        if vertex % 4096 == 0 {
            ctx.checkpoint()?;
        }
        vertices.positions.push(reader.vec3()?);
        vertices.normals.push(reader.vec3()?);
        vertices.uvs.push(reader.vec2()?);
        for uvs in &mut extra.additional_uvs {
            uvs.push(reader.vec4()?);
        }

        let kind = reader.u8()?;
        let bone = |reader: &mut ByteReader<'_>| -> Result<u16> {
            let bone_index = index(reader, header.sizes.bone)?.unwrap_or(0);
            u16::try_from(bone_index).map_err(|_| {
                Error::Damaged(format!(
                    "vertex bone index {bone_index} out of range for {} bones",
                    u16::MAX
                ))
            })
        };
        let kind = match kind {
            0 => {
                vertices.set_influences(vertex, &[(bone(reader)?, 1.0)]);
                PmxWeight::One
            }
            1 | 3 => {
                let (first, second) = (bone(reader)?, bone(reader)?);
                let weight = reader.f32()?;
                vertices.set_influences(vertex, &[(first, weight), (second, 1.0 - weight)]);
                match kind {
                    1 => PmxWeight::Two,
                    _ => {
                        extra.spherical.push(PmxSpherical {
                            vertex: vertex as u32,
                            centre: reader.vec3()?,
                            radius0: reader.vec3()?,
                            radius1: reader.vec3()?,
                        });
                        PmxWeight::Spherical
                    }
                }
            }
            2 | 4 if kind == 2 || header.version == PmxVersion::V2_1 => {
                let joints = [bone(reader)?, bone(reader)?, bone(reader)?, bone(reader)?];
                let weights = reader.vec4()?.to_array();
                let influences: [(u16, f32); INFLUENCES] =
                    std::array::from_fn(|slot| (joints[slot], weights[slot]));
                vertices.set_influences(vertex, &influences);
                match kind {
                    2 => PmxWeight::Four,
                    _ => PmxWeight::Quaternion,
                }
            }
            other => {
                return Err(Error::Damaged(format!(
                    "vertex {vertex} has unknown weight deform type {other}"
                )));
            }
        };
        extra.weights.push(kind);
        extra.edge_scales.push(reader.f32()?);
    }
    Ok((vertices, extra))
}

fn read_faces(
    reader: &mut ByteReader<'_>,
    header: &Header,
    vertices: usize,
    ctx: &mut ImportCtx<'_>,
) -> Result<Vec<u32>> {
    let count = reader.count(usize::from(header.sizes.vertex), "face indices")?;
    if !count.is_multiple_of(3) {
        return Err(Error::Damaged(format!(
            "face index count {count} is not a multiple of 3"
        )));
    }
    let mut indices = Vec::with_capacity(count);
    for corner in 0..count {
        if corner % 8192 == 0 {
            ctx.checkpoint()?;
        }
        let index = vertex_index(reader, header.sizes.vertex)?;
        if index as usize >= vertices {
            return Err(Error::Damaged(format!(
                "face index {index} out of range for {vertices} vertices"
            )));
        }
        indices.push(index);
    }
    Ok(indices)
}

fn read_textures(reader: &mut ByteReader<'_>, header: &Header) -> Result<Vec<String>> {
    let count = reader.count(4, "textures")?;
    let mut textures = Vec::with_capacity(count);
    for _ in 0..count {
        textures.push(header.encoding.read(reader)?);
    }
    Ok(textures)
}

fn read_materials(
    reader: &mut ByteReader<'_>,
    header: &Header,
    textures: &[String],
) -> Result<(Vec<PmxMaterial>, Vec<u32>)> {
    let count = reader.count(MATERIAL, "materials")?;
    let mut materials = Vec::with_capacity(count);
    let mut counts = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, english_name) = named(reader, header)?;
        let diffuse = reader.vec4()?;
        let specular = reader.vec3()?;
        let specular_power = reader.f32()?;
        let ambient = reader.vec3()?;
        let flags = PmxMaterialFlags(match header.version {
            PmxVersion::V2_0 => reader.u8()? & !PmxMaterialFlags::PMX_2_1,
            PmxVersion::V2_1 => reader.u8()?,
        });
        let edge_color = reader.vec4()?;
        let edge_size = reader.f32()?;
        let known = |texture: Option<usize>| texture.filter(|texture| *texture < textures.len());
        let texture = known(index(reader, header.sizes.texture)?);
        let sphere = known(index(reader, header.sizes.texture)?);
        let sphere_mode = PmxSphereMode::from_byte(reader.u8()?);
        let toon = match reader.u8()? {
            0 => PmxToon::Texture(known(index(reader, header.sizes.texture)?)),
            _ => PmxToon::Shared(reader.u8()?),
        };
        let memo = header.encoding.read(reader)?;
        let indices = reader.i32()?;
        if indices < 0 || !(indices as usize).is_multiple_of(3) {
            return Err(Error::Damaged(format!(
                "material \"{name}\": {indices} indices is not a multiple of 3"
            )));
        }
        counts.push(indices as u32);
        materials.push(PmxMaterial {
            name,
            english_name,
            diffuse,
            specular,
            specular_power,
            ambient,
            flags,
            edge_color,
            edge_size,
            texture,
            sphere,
            sphere_mode,
            toon,
            memo,
        });
    }
    Ok((materials, counts))
}

fn read_bones(reader: &mut ByteReader<'_>, header: &Header) -> Result<Vec<PmxBone>> {
    let count = reader.count(BONE, "bones")?;
    let mut bones = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, english_name) = named(reader, header)?;
        let position = reader.vec3()?;
        let parent = index(reader, header.sizes.bone)?;
        let layer = reader.i32()?;
        let flags = PmxBoneFlags(reader.u16()?);

        let tail = match flags.has(PmxBoneFlags::TAIL_IS_BONE) {
            true => PmxTail::Bone(index(reader, header.sizes.bone)?),
            false => PmxTail::Offset(reader.vec3()?),
        };
        let appends = PmxBoneFlags::APPEND_ROTATE | PmxBoneFlags::APPEND_TRANSLATE;
        let append = match flags.has(appends) {
            true => {
                let parent = index(reader, header.sizes.bone)?;
                let ratio = reader.f32()?;
                parent.map(|parent| PmxAppend { parent, ratio })
            }
            false => None,
        };
        let fixed_axis = match flags.has(PmxBoneFlags::FIXED_AXIS) {
            true => Some(reader.vec3()?.normalize_or(Vec3::X)),
            false => None,
        };
        let local_axes = match flags.has(PmxBoneFlags::LOCAL_AXES) {
            true => Some((reader.vec3()?, reader.vec3()?)),
            false => None,
        };
        let external_parent = match flags.has(PmxBoneFlags::EXTERNAL_PARENT) {
            true => Some(reader.i32()?),
            false => None,
        };
        let ik = match flags.has(PmxBoneFlags::IK) {
            true => Some(read_ik(reader, header)?),
            false => None,
        };

        bones.push(PmxBone {
            name,
            english_name,
            node: NodeId(0),
            parent,
            layer,
            flags,
            tail,
            append,
            fixed_axis,
            local_axes,
            external_parent,
            ik,
            position,
        });
    }
    Ok(bones)
}

fn read_ik(reader: &mut ByteReader<'_>, header: &Header) -> Result<PmxIk> {
    let target = index(reader, header.sizes.bone)?.unwrap_or(0);
    let loops = reader.i32()?.max(0) as u32;
    let limit_angle = reader.f32()?;
    let count = reader.count(IK_LINK, "IK links")?;
    let mut links = Vec::with_capacity(count);
    for _ in 0..count {
        let bone = index(reader, header.sizes.bone)?;
        let limits = match reader.u8()? {
            0 => None,
            _ => Some(PmxLimits {
                lower: reader.vec3()?,
                upper: reader.vec3()?,
            }),
        };
        if let Some(bone) = bone {
            links.push(PmxIkLink { bone, limits });
        }
    }
    Ok(PmxIk {
        target,
        loops,
        limit_angle,
        links,
    })
}

fn read_morphs(
    reader: &mut ByteReader<'_>,
    header: &Header,
    vertices: &Vertices,
) -> Result<(Vec<PmxMorph>, Vec<MorphTarget>)> {
    let count = reader.count(MORPH, "morphs")?;
    let mut morphs = Vec::with_capacity(count);
    let mut targets = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, english_name) = named(reader, header)?;
        let panel = PmxPanel::from_byte(reader.u8()?);
        let kind = reader.u8()?;
        let offsets = reader.count(MORPH_OFFSET, "morph offsets")?;
        let mut target = MorphTarget::new(name.clone());

        let offsets = match kind {
            0 => PmxOffsets::Group(read_group_offsets(reader, header, offsets, count)?),
            1 => {
                read_vertex_offsets(reader, header, offsets, vertices.len(), &mut target)?;
                PmxOffsets::Vertex
            }
            2 => PmxOffsets::Bone(read_bone_offsets(reader, header, offsets)?),
            3..=7 => PmxOffsets::Uv {
                channel: kind - 3,
                offsets: read_uv_offsets(reader, header, offsets, vertices.len())?,
            },
            8 => PmxOffsets::Material(read_material_offsets(reader, header, offsets)?),
            9 if header.version == PmxVersion::V2_1 => {
                PmxOffsets::Flip(read_group_offsets(reader, header, offsets, count)?)
            }
            10 if header.version == PmxVersion::V2_1 => {
                PmxOffsets::Impulse(read_impulse_offsets(reader, header, offsets)?)
            }
            other => {
                return Err(Error::Damaged(format!(
                    "morph \"{name}\" has unknown kind {other}"
                )));
            }
        };
        targets.push(target);
        morphs.push(PmxMorph {
            name,
            english_name,
            panel,
            offsets,
        });
    }
    Ok((morphs, targets))
}

fn read_group_offsets(
    reader: &mut ByteReader<'_>,
    header: &Header,
    count: usize,
    morphs: usize,
) -> Result<Vec<PmxGroupOffset>> {
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        let morph = index(reader, header.sizes.morph)?;
        let ratio = reader.f32()?;
        if let Some(morph) = morph.filter(|morph| *morph < morphs) {
            offsets.push(PmxGroupOffset { morph, ratio });
        }
    }
    Ok(offsets)
}

fn read_vertex_offsets(
    reader: &mut ByteReader<'_>,
    header: &Header,
    count: usize,
    vertices: usize,
    target: &mut MorphTarget,
) -> Result<()> {
    target.vertices.reserve(count);
    target.positions.reserve(count);
    for _ in 0..count {
        let vertex = vertex_index(reader, header.sizes.vertex)?;
        let offset = reader.vec3()?;
        if (vertex as usize) < vertices {
            target.push(vertex, offset, None);
        }
    }
    target.merge(Repeats::Sum);
    Ok(())
}

fn read_bone_offsets(
    reader: &mut ByteReader<'_>,
    header: &Header,
    count: usize,
) -> Result<Vec<PmxBoneOffset>> {
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        let bone = index(reader, header.sizes.bone)?;
        let translation = reader.vec3()?;
        let rotation = rikka_model_core::glam::Quat::from_vec4(reader.vec4()?).normalize();
        if let Some(bone) = bone {
            offsets.push(PmxBoneOffset {
                bone,
                translation,
                rotation,
            });
        }
    }
    Ok(offsets)
}

fn read_uv_offsets(
    reader: &mut ByteReader<'_>,
    header: &Header,
    count: usize,
    vertices: usize,
) -> Result<Vec<PmxUvOffset>> {
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        let vertex = vertex_index(reader, header.sizes.vertex)?;
        let offset = reader.vec4()?;
        if (vertex as usize) < vertices {
            offsets.push(PmxUvOffset { vertex, offset });
        }
    }
    Ok(offsets)
}

fn read_material_offsets(
    reader: &mut ByteReader<'_>,
    header: &Header,
    count: usize,
) -> Result<Vec<PmxMaterialOffset>> {
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        let material = index(reader, header.sizes.material)?;
        let blend = match reader.u8()? {
            0 => PmxBlend::Multiply,
            _ => PmxBlend::Add,
        };
        offsets.push(PmxMaterialOffset {
            material,
            blend,
            diffuse: reader.vec4()?,
            specular: reader.vec3()?,
            specular_power: reader.f32()?,
            ambient: reader.vec3()?,
            edge_color: reader.vec4()?,
            edge_size: reader.f32()?,
            texture_tint: reader.vec4()?,
            sphere_tint: reader.vec4()?,
            toon_tint: reader.vec4()?,
        });
    }
    Ok(offsets)
}

fn read_display_frames(
    reader: &mut ByteReader<'_>,
    header: &Header,
) -> Result<Vec<PmxDisplayFrame>> {
    let count = reader.count(DISPLAY_FRAME, "display frames")?;
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, english_name) = named(reader, header)?;
        let special = reader.u8()? != 0;
        let targets = reader.count(DISPLAY_TARGET, "display frame targets")?;
        let mut frame_targets = Vec::with_capacity(targets);
        for _ in 0..targets {
            let target = match reader.u8()? {
                0 => index(reader, header.sizes.bone)?.map(PmxDisplayTarget::Bone),
                _ => index(reader, header.sizes.morph)?.map(PmxDisplayTarget::Morph),
            };
            frame_targets.extend(target);
        }
        frames.push(PmxDisplayFrame {
            name,
            english_name,
            special,
            targets: frame_targets,
        });
    }
    Ok(frames)
}

fn read_rigid_bodies(reader: &mut ByteReader<'_>, header: &Header) -> Result<Vec<PmxRigidBody>> {
    let count = reader.count(RIGID_BODY, "rigid bodies")?;
    let mut bodies = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, english_name) = named(reader, header)?;
        bodies.push(PmxRigidBody {
            name,
            english_name,
            bone: index(reader, header.sizes.bone)?,
            group: reader.u8()?,
            collision_mask: reader.u16()?,
            shape: PmxShape::from_byte(reader.u8()?),
            size: reader.vec3()?,
            position: reader.vec3()?,
            rotation: euler::rotation_of(reader.vec3()?),
            mass: reader.f32()?,
            linear_damping: reader.f32()?,
            angular_damping: reader.f32()?,
            restitution: reader.f32()?,
            friction: reader.f32()?,
            physics: PmxPhysics::from_byte(reader.u8()?),
        });
    }
    Ok(bodies)
}

fn read_joints(reader: &mut ByteReader<'_>, header: &Header) -> Result<Vec<PmxJoint>> {
    let count = reader.count(JOINT, "joints")?;
    let mut joints = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, english_name) = named(reader, header)?;
        let kind = joint_kind(reader.u8()?, header.version)?;
        let bodies = (
            index(reader, header.sizes.rigid_body)?,
            index(reader, header.sizes.rigid_body)?,
        );
        joints.push(PmxJoint {
            name,
            english_name,
            kind,
            bodies,
            position: reader.vec3()?,
            rotation: euler::rotation_of(reader.vec3()?),
            translation_limits: PmxLimits {
                lower: reader.vec3()?,
                upper: reader.vec3()?,
            },
            rotation_limits: PmxLimits {
                lower: reader.vec3()?,
                upper: reader.vec3()?,
            },
            translation_spring: reader.vec3()?,
            rotation_spring: reader.vec3()?,
        });
    }
    Ok(joints)
}

fn joint_kind(byte: u8, version: PmxVersion) -> Result<u8> {
    match version {
        PmxVersion::V2_0 => Ok(PmxJointKind::Spring6Dof.byte()),
        PmxVersion::V2_1 => PmxJointKind::from_byte(byte)
            .map(PmxJointKind::byte)
            .ok_or_else(|| Error::Damaged(format!("unknown joint kind {byte}"))),
    }
}

fn read_impulse_offsets(
    reader: &mut ByteReader<'_>,
    header: &Header,
    count: usize,
) -> Result<Vec<PmxImpulseOffset>> {
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        let body = index(reader, header.sizes.rigid_body)?;
        let local = reader.u8()? != 0;
        let velocity = reader.vec3()?;
        let torque = reader.vec3()?;
        if let Some(body) = body {
            offsets.push(PmxImpulseOffset {
                body,
                local,
                velocity,
                torque,
            });
        }
    }
    Ok(offsets)
}

fn drop_dangling_references(pmx: &mut Pmx) {
    let bones = pmx.bones.len();
    let morphs = pmx.morphs.len();
    let materials = pmx.materials.len();
    let bodies = pmx.rigid_bodies.len();
    let bone = |index: Option<usize>| index.filter(|index| *index < bones);
    let body = |index: Option<usize>| index.filter(|index| *index < bodies);
    for pmx_bone in &mut pmx.bones {
        pmx_bone.parent = bone(pmx_bone.parent);
        if let PmxTail::Bone(tail) = &mut pmx_bone.tail {
            *tail = bone(*tail);
        }
        pmx_bone.append = pmx_bone.append.filter(|append| append.parent < bones);
        if pmx_bone.ik.as_ref().is_some_and(|ik| ik.target >= bones) {
            pmx_bone.ik = None;
        }
        if let Some(ik) = &mut pmx_bone.ik {
            ik.links.retain(|link| link.bone < bones);
        }
    }
    for morph in &mut pmx.morphs {
        match &mut morph.offsets {
            PmxOffsets::Bone(offsets) => offsets.retain(|offset| offset.bone < bones),
            PmxOffsets::Material(offsets) => {
                offsets.retain(|offset| offset.material.is_none_or(|material| material < materials))
            }
            PmxOffsets::Impulse(offsets) => offsets.retain(|offset| offset.body < bodies),
            PmxOffsets::Group(_)
            | PmxOffsets::Vertex
            | PmxOffsets::Uv { .. }
            | PmxOffsets::Flip(_) => {}
        }
    }
    for frame in &mut pmx.display_frames {
        frame.targets.retain(|target| match target {
            PmxDisplayTarget::Bone(index) => *index < bones,
            PmxDisplayTarget::Morph(index) => *index < morphs,
        });
    }
    for rigid_body in &mut pmx.rigid_bodies {
        rigid_body.bone = bone(rigid_body.bone);
    }
    for joint in &mut pmx.joints {
        joint.bodies = (body(joint.bodies.0), body(joint.bodies.1));
    }
}

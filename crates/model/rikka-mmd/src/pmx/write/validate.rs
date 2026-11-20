use rikka_model_core::{Error, Result};

use crate::pmx::document::{
    Pmx, PmxDisplayTarget, PmxJointKind, PmxMaterial, PmxOffsets, PmxTail, PmxToon,
};
use crate::pmx::soft_body::PmxSoftBody;

const ADDITIONAL_UVS: usize = 4;

struct Counts {
    vertices: usize,
    textures: usize,
    materials: usize,
    bones: usize,
    morphs: usize,
    rigid_bodies: usize,
}

pub(super) fn validate(pmx: &Pmx, vertices: usize) -> Result<()> {
    let counts = Counts {
        vertices,
        textures: pmx.textures.len(),
        materials: pmx.materials.len(),
        bones: pmx.bones.len(),
        morphs: pmx.morphs.len(),
        rigid_bodies: pmx.rigid_bodies.len(),
    };
    validate_vertices(pmx, &counts)?;
    for material in &pmx.materials {
        validate_material(material, &counts)?;
    }
    validate_bones(pmx, &counts)?;
    validate_morphs(pmx, &counts)?;
    validate_frames(pmx, &counts)?;
    validate_physics(pmx, &counts)?;
    for soft_body in &pmx.soft_bodies {
        validate_soft_body(soft_body, &counts)?;
    }
    Ok(())
}

fn out_of_range(what: String, index: usize, count: usize, of: &str) -> Error {
    Error::Invalid(format!(
        "PMX {what}: index {index} out of range for {count} {of}"
    ))
}

fn check(index: usize, count: usize, what: impl FnOnce() -> String, of: &str) -> Result<()> {
    match index < count {
        true => Ok(()),
        false => Err(out_of_range(what(), index, count, of)),
    }
}

fn validate_vertices(pmx: &Pmx, counts: &Counts) -> Result<()> {
    let uvs = &pmx.vertices.additional_uvs;
    if uvs.len() > ADDITIONAL_UVS {
        return Err(Error::Invalid(format!(
            "PMX holds at most {ADDITIONAL_UVS} additional UV sets, not {}",
            uvs.len()
        )));
    }
    for (set, values) in uvs.iter().enumerate() {
        if values.len() != counts.vertices {
            return Err(Error::Invalid(format!(
                "PMX additional UV set {} has {} values for {} vertices",
                set + 1,
                values.len(),
                counts.vertices
            )));
        }
    }
    for sphere in &pmx.vertices.spherical {
        check(
            sphere.vertex as usize,
            counts.vertices,
            || "SDEF vertex".to_owned(),
            "vertices",
        )?;
    }
    Ok(())
}

fn validate_material(material: &PmxMaterial, counts: &Counts) -> Result<()> {
    let toon = match material.toon {
        PmxToon::Texture(texture) => texture,
        PmxToon::Shared(_) => None,
    };
    for texture in [material.texture, material.sphere, toon]
        .into_iter()
        .flatten()
    {
        check(
            texture,
            counts.textures,
            || format!("material \"{}\" texture", material.name),
            "textures",
        )?;
    }
    Ok(())
}

fn validate_bones(pmx: &Pmx, counts: &Counts) -> Result<()> {
    for bone in &pmx.bones {
        let what = |field: &str| format!("bone \"{}\" {field}", bone.name);
        if let Some(parent) = bone.parent {
            check(parent, counts.bones, || what("parent"), "bones")?;
        }
        if let PmxTail::Bone(Some(tail)) = bone.tail {
            check(tail, counts.bones, || what("tail"), "bones")?;
        }
        if let Some(append) = bone.append {
            check(
                append.parent,
                counts.bones,
                || what("append parent"),
                "bones",
            )?;
        }
        if let Some(ik) = &bone.ik {
            check(ik.target, counts.bones, || what("IK target"), "bones")?;
            for link in &ik.links {
                check(link.bone, counts.bones, || what("IK link"), "bones")?;
            }
        }
    }
    Ok(())
}

fn validate_morphs(pmx: &Pmx, counts: &Counts) -> Result<()> {
    for morph in &pmx.morphs {
        let what = || format!("morph \"{}\"", morph.name);
        match &morph.offsets {
            PmxOffsets::Group(offsets) | PmxOffsets::Flip(offsets) => {
                for offset in offsets {
                    check(offset.morph, counts.morphs, what, "morphs")?;
                }
            }
            PmxOffsets::Vertex => {}
            PmxOffsets::Bone(offsets) => {
                for offset in offsets {
                    check(offset.bone, counts.bones, what, "bones")?;
                }
            }
            PmxOffsets::Uv { channel, offsets } => {
                if usize::from(*channel) > ADDITIONAL_UVS {
                    return Err(Error::Invalid(format!(
                        "PMX {}: UV channel {channel} is past the {ADDITIONAL_UVS} additional sets",
                        what()
                    )));
                }
                for offset in offsets {
                    check(offset.vertex as usize, counts.vertices, what, "vertices")?;
                }
            }
            PmxOffsets::Material(offsets) => {
                for material in offsets.iter().filter_map(|offset| offset.material) {
                    check(material, counts.materials, what, "materials")?;
                }
            }
            PmxOffsets::Impulse(offsets) => {
                for offset in offsets {
                    check(offset.body, counts.rigid_bodies, what, "rigid bodies")?;
                }
            }
        }
    }
    Ok(())
}

fn validate_frames(pmx: &Pmx, counts: &Counts) -> Result<()> {
    for frame in &pmx.display_frames {
        let what = || format!("display frame \"{}\"", frame.name);
        for target in &frame.targets {
            match *target {
                PmxDisplayTarget::Bone(bone) => check(bone, counts.bones, what, "bones")?,
                PmxDisplayTarget::Morph(morph) => check(morph, counts.morphs, what, "morphs")?,
            }
        }
    }
    Ok(())
}

fn validate_physics(pmx: &Pmx, counts: &Counts) -> Result<()> {
    for body in &pmx.rigid_bodies {
        if let Some(bone) = body.bone {
            check(
                bone,
                counts.bones,
                || format!("rigid body \"{}\" bone", body.name),
                "bones",
            )?;
        }
    }
    for joint in &pmx.joints {
        let what = || format!("joint \"{}\"", joint.name);
        if PmxJointKind::from_byte(joint.kind).is_none() {
            return Err(Error::Invalid(format!(
                "PMX {}: unknown joint kind {}",
                what(),
                joint.kind
            )));
        }
        for body in [joint.bodies.0, joint.bodies.1].into_iter().flatten() {
            check(body, counts.rigid_bodies, what, "rigid bodies")?;
        }
    }
    Ok(())
}

fn validate_soft_body(soft_body: &PmxSoftBody, counts: &Counts) -> Result<()> {
    let what = |field: &str| format!("soft body \"{}\" {field}", soft_body.name);
    if let Some(material) = soft_body.material {
        check(material, counts.materials, || what("material"), "materials")?;
    }
    for anchor in &soft_body.anchors {
        check(
            anchor.body,
            counts.rigid_bodies,
            || what("anchor"),
            "rigid bodies",
        )?;
        check(
            anchor.vertex as usize,
            counts.vertices,
            || what("anchor"),
            "vertices",
        )?;
    }
    for pin in &soft_body.pins {
        check(*pin as usize, counts.vertices, || what("pin"), "vertices")?;
    }
    Ok(())
}

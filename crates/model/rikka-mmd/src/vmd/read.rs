use rikka_model_core::glam::Quat;

use rikka_model_core::ImportCtx;
use rikka_model_core::{Error, Result};

use super::super::text::decode_name;
use super::document::{
    FRAME_NAME, HEADER, IK_NAME, LEGACY_NAME, LEGACY_SIGNATURE, NAME, PHYSICS_OFF, SIGNATURE, Vmd,
    VmdBoneFrame, VmdCameraFrame, VmdCameraInterpolation, VmdCurve, VmdInterpolation,
    VmdLightFrame, VmdMorphFrame, VmdPropertyFrame, VmdShadowFrame,
};
use rikka_model_core::ByteReader;

pub(crate) fn sniffs(head: &[u8]) -> bool {
    head.starts_with(SIGNATURE) || head.starts_with(LEGACY_SIGNATURE)
}

pub(crate) fn read(bytes: &[u8], ctx: &mut ImportCtx<'_>) -> Result<Vmd> {
    let mut reader = ByteReader::new(bytes);
    let header = reader.take(HEADER)?;
    let legacy = if header.starts_with(SIGNATURE) {
        false
    } else if header.starts_with(LEGACY_SIGNATURE) {
        true
    } else {
        return Err(Error::Damaged("missing VMD magic".to_owned()));
    };

    let mut motion = Vmd {
        model: decode_name(reader.take(match legacy {
            true => LEGACY_NAME,
            false => NAME,
        })?),
        ..Vmd::default()
    };

    read_bones(&mut reader, &mut motion, ctx)?;
    read_morphs(&mut reader, &mut motion, ctx)?;
    if reader.is_empty() {
        return Ok(motion);
    }
    read_cameras(&mut reader, &mut motion, legacy)?;
    if reader.is_empty() {
        return Ok(motion);
    }
    read_lights(&mut reader, &mut motion)?;
    if legacy || reader.is_empty() {
        return Ok(motion);
    }
    read_shadows(&mut reader, &mut motion)?;
    if reader.is_empty() {
        return Ok(motion);
    }
    read_properties(&mut reader, &mut motion)?;
    Ok(motion)
}

fn read_bones(
    reader: &mut ByteReader<'_>,
    motion: &mut Vmd,
    ctx: &mut ImportCtx<'_>,
) -> Result<()> {
    let count = reader.count(111, "bone keyframes")?;
    motion.bones.reserve(count);
    for index in 0..count {
        if index % 4096 == 0 {
            ctx.checkpoint()?;
        }
        let bone = decode_name(reader.take(FRAME_NAME)?);
        let frame = reader.u32()?;
        let translation = reader.vec3()?;
        let rotation = Quat::from_vec4(reader.vec4()?);
        let interpolation_bytes = reader.take(64)?;
        let (interpolation, physics_off) = interpolation_of(interpolation_bytes);
        motion.bones.push(VmdBoneFrame {
            bone,
            frame,
            translation,
            rotation: match rotation.length_squared() > 1e-12 {
                true => rotation.normalize(),
                false => Quat::IDENTITY,
            },
            interpolation,
            physics_off,
        });
    }
    Ok(())
}

fn interpolation_of(bytes: &[u8]) -> (VmdInterpolation, bool) {
    let mut curves = [VmdCurve::LINEAR; 4];
    for (curve, slot_curve) in curves.iter_mut().enumerate() {
        let mut parameters = [0_u8; 4];
        for (parameter, slot) in parameters.iter_mut().enumerate() {
            *slot = bytes[(curve * 4 + parameter) * 4];
        }
        *slot_curve = VmdCurve::from_bytes(parameters);
    }
    let marked = u16::from_le_bytes([bytes[2], bytes[3]]) == PHYSICS_OFF;
    (VmdInterpolation::from_curves(curves), marked)
}

fn read_morphs(
    reader: &mut ByteReader<'_>,
    motion: &mut Vmd,
    ctx: &mut ImportCtx<'_>,
) -> Result<()> {
    let count = reader.count(23, "morph keyframes")?;
    motion.morphs.reserve(count);
    for index in 0..count {
        if index % 8192 == 0 {
            ctx.checkpoint()?;
        }
        let morph = decode_name(reader.take(FRAME_NAME)?);
        let frame = reader.u32()?;
        let weight = reader.f32()?;
        motion.morphs.push(VmdMorphFrame {
            morph,
            frame,
            weight: match weight.is_finite() {
                true => weight,
                false => 0.0,
            },
        });
    }
    Ok(())
}

fn read_cameras(reader: &mut ByteReader<'_>, motion: &mut Vmd, legacy: bool) -> Result<()> {
    let stride = match legacy {
        true => 36,
        false => 61,
    };
    let count = reader.count(stride, "camera keyframes")?;
    motion.cameras.reserve(count);
    for _ in 0..count {
        let frame = reader.u32()?;
        let distance = reader.f32()?;
        let target = reader.vec3()?;
        let rotation = reader.vec3()?;
        let (interpolation, angle, perspective) = match legacy {
            true => {
                let curve_bytes = reader.array::<4>()?;
                (
                    VmdCameraInterpolation::from_curves(
                        [VmdCurve::from_bytes([
                            curve_bytes[0],
                            curve_bytes[2],
                            curve_bytes[1],
                            curve_bytes[3],
                        ]); 6],
                    ),
                    45,
                    true,
                )
            }
            false => {
                let interpolation_bytes = reader.take(24)?;
                let mut curves = [VmdCurve::LINEAR; 6];
                for (curve, slot) in curves.iter_mut().enumerate() {
                    let curve_bytes = &interpolation_bytes[curve * 4..curve * 4 + 4];
                    *slot = VmdCurve::from_bytes([
                        curve_bytes[0],
                        curve_bytes[2],
                        curve_bytes[1],
                        curve_bytes[3],
                    ]);
                }
                let interpolation = VmdCameraInterpolation::from_curves(curves);
                (interpolation, reader.u32()?, reader.u8()? == 0)
            }
        };
        motion.cameras.push(VmdCameraFrame {
            frame,
            distance,
            target,
            rotation,
            interpolation,
            angle,
            perspective,
        });
    }
    Ok(())
}

fn read_lights(reader: &mut ByteReader<'_>, motion: &mut Vmd) -> Result<()> {
    let count = reader.count(28, "light keyframes")?;
    motion.lights.reserve(count);
    for _ in 0..count {
        motion.lights.push(VmdLightFrame {
            frame: reader.u32()?,
            color: reader.vec3()?,
            direction: reader.vec3()?,
        });
    }
    Ok(())
}

fn read_shadows(reader: &mut ByteReader<'_>, motion: &mut Vmd) -> Result<()> {
    let count = reader.count(9, "shadow keyframes")?;
    motion.shadows.reserve(count);
    for _ in 0..count {
        motion.shadows.push(VmdShadowFrame {
            frame: reader.u32()?,
            mode: reader.u8()?,
            distance: reader.f32()?,
        });
    }
    Ok(())
}

fn read_properties(reader: &mut ByteReader<'_>, motion: &mut Vmd) -> Result<()> {
    let count = reader.count(9, "property keyframes")?;
    motion.properties.reserve(count);
    for _ in 0..count {
        let frame = reader.u32()?;
        let visible = reader.u8()? != 0;
        let ik_count = reader.count(IK_NAME + 1, "IK bones")?;
        let mut ik = Vec::with_capacity(ik_count);
        for _ in 0..ik_count {
            let name = decode_name(reader.take(IK_NAME)?);
            ik.push((name, reader.u8()? != 0));
        }
        motion
            .properties
            .push(VmdPropertyFrame { frame, visible, ik });
    }
    Ok(())
}

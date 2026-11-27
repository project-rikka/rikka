use rikka_model_core::glam::{Quat, Vec3};
use rustc_hash::FxHashMap;

use rikka_model_core::{Error, Result};

use super::super::text::decode_name;
use super::document::{
    FRAME_NAME, HEADER, IK_NAME, LEGACY_NAME, LEGACY_SIGNATURE, NAME, PHYSICS_OFF, SIGNATURE, Vmd,
    VmdBoneFrame, VmdCameraFrame, VmdCameraInterpolation, VmdCurve, VmdInterpolation,
    VmdLightFrame, VmdMorphFrame, VmdPropertyFrame, VmdShadowFrame,
};
use rikka_model_core::ByteReader;

const BONE_RECORD: usize = 111;
const MORPH_RECORD: usize = 23;
const BONES_PER_CHECKPOINT: usize = 4096;
const MORPHS_PER_CHECKPOINT: usize = 8192;

#[derive(Default)]
struct Names {
    decoded: FxHashMap<[u8; FRAME_NAME], String>,
}

impl Names {
    fn decode(&mut self, bytes: &[u8; FRAME_NAME]) -> String {
        self.decoded
            .entry(*bytes)
            .or_insert_with(|| decode_name(bytes))
            .clone()
    }
}

fn word(bytes: &[u8], at: usize) -> [u8; 4] {
    [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
}

fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(word(bytes, at))
}

fn vector(bytes: &[u8], at: usize) -> Vec3 {
    Vec3::new(float(bytes, at), float(bytes, at + 4), float(bytes, at + 8))
}

fn name_of(record: &[u8]) -> &[u8; FRAME_NAME] {
    record[..FRAME_NAME]
        .try_into()
        .expect("a record starts with its name")
}

pub(crate) fn sniffs(head: &[u8]) -> bool {
    head.starts_with(SIGNATURE) || head.starts_with(LEGACY_SIGNATURE)
}

pub(crate) fn read(bytes: &[u8], checkpoint: &dyn Fn() -> Result<()>) -> Result<Vmd> {
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

    let mut names = Names::default();
    read_bones(&mut reader, &mut motion, &mut names, checkpoint)?;
    read_morphs(&mut reader, &mut motion, &mut names, checkpoint)?;
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
    names: &mut Names,
    checkpoint: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let count = reader.count(BONE_RECORD, "bone keyframes")?;
    let records = reader
        .take(count * BONE_RECORD)?
        .as_chunks::<BONE_RECORD>()
        .0;
    motion.bones.reserve(count);
    for (index, record) in records.iter().enumerate() {
        if index % BONES_PER_CHECKPOINT == 0 {
            checkpoint()?;
        }
        let rotation = Quat::from_xyzw(
            float(record, 31),
            float(record, 35),
            float(record, 39),
            float(record, 43),
        );
        let (interpolation, physics_off) = interpolation_of(&record[47..]);
        motion.bones.push(VmdBoneFrame {
            bone: names.decode(name_of(record)),
            frame: u32::from_le_bytes(word(record, 15)),
            translation: vector(record, 19),
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
    names: &mut Names,
    checkpoint: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let count = reader.count(MORPH_RECORD, "morph keyframes")?;
    let records = reader
        .take(count * MORPH_RECORD)?
        .as_chunks::<MORPH_RECORD>()
        .0;
    motion.morphs.reserve(count);
    for (index, record) in records.iter().enumerate() {
        if index % MORPHS_PER_CHECKPOINT == 0 {
            checkpoint()?;
        }
        let weight = float(record, 19);
        motion.morphs.push(VmdMorphFrame {
            morph: names.decode(name_of(record)),
            frame: u32::from_le_bytes(word(record, 15)),
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

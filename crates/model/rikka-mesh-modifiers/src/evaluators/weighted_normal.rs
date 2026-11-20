use std::cmp::Ordering;
use std::f32::consts::PI;

use glam::Vec3;
use rikka_polymesh::{Domain, Polymesh};

use crate::kinds::weighted_normal::{WeightedNormal, WeightedNormalMode};
use crate::math::normalize;

pub(crate) const FACE_STRENGTH: &str = "__mod_weightednormals_faceweight";
const FACE_STRENGTH_WEAK: i32 = -16384;
const VALID_LENGTH: f32 = 1.0e-6;
const CACHED_POWERS: usize = 128;

#[derive(Clone, Copy)]
struct Item {
    normal: Vec3,
    loops: i32,
    value: f32,
    strength: i32,
}

struct Weighting<'a> {
    settings: &'a WeightedNormal,
    weight: f32,
    face_normals: &'a [Vec3],
    strengths: Option<&'a [i32]>,
    members: Option<Vec<bool>>,
    powers: [f32; CACHED_POWERS],
}

impl Weighting<'_> {
    fn aggregate(&mut self, item: &mut Item, point: usize, face: usize, value: f32) {
        if let Some(members) = &self.members
            && members[point] == self.settings.invert_vertex_group
        {
            return;
        }
        if let Some(strengths) = self.strengths {
            let strength = strengths[face];
            if strength > item.strength {
                item.strength = strength;
                item.value = 0.0;
                item.loops = 0;
                item.normal = Vec3::ZERO;
            }
            if strength != item.strength {
                return;
            }
        }
        if item.value == 0.0 {
            item.value = value;
        }
        if (item.value - value).abs() > self.settings.threshold {
            item.loops += 1;
            item.value = value;
        }
        let loops = item.loops as usize;
        let inverse = match loops < CACHED_POWERS {
            true => {
                if self.powers[loops] == 0.0 {
                    self.powers[loops] = 1.0 / self.weight.powf(loops as f32);
                }
                self.powers[loops]
            }
            false => 1.0 / self.weight.powf(loops as f32),
        };
        item.normal += self.face_normals[face] * (value * inverse);
    }
}

fn weight(setting: u8) -> f32 {
    let weight = f32::from(setting) / 50.0;
    match setting {
        100 => f32::from(i16::MAX),
        1 => 1.0 / f32::from(i16::MAX),
        _ if (weight - 1.0) * 25.0 > 1.0 => (weight - 1.0) * 25.0,
        _ => weight,
    }
}

fn face_area(mesh: &Polymesh, face: usize) -> f32 {
    let verts = mesh.face_verts(face);
    let at = |index: usize| mesh.positions[verts[index] as usize];
    if verts.len() == 3 {
        return (at(0) - at(1)).cross(at(1) - at(2)).length() * 0.5;
    }
    let mut sum = Vec3::ZERO;
    let mut previous = at(verts.len() - 1);
    for index in 0..verts.len() {
        let current = at(index);
        sum.x += (previous.y - current.y) * (previous.z + current.z);
        sum.y += (previous.z - current.z) * (previous.x + current.x);
        sum.z += (previous.x - current.x) * (previous.y + current.y);
        previous = current;
    }
    sum.length() * 0.5
}

fn angle_normalized(a: Vec3, b: Vec3) -> f32 {
    let safe_asin = |x: f32| match x {
        x if x <= -1.0 => -PI / 2.0,
        x if x >= 1.0 => PI / 2.0,
        x => x.asin(),
    };
    match a.dot(b) >= 0.0 {
        true => 2.0 * safe_asin((a - b).length() / 2.0),
        false => PI - 2.0 * safe_asin((a + b).length() / 2.0),
    }
}

fn face_angles(mesh: &Polymesh, face: usize) -> Vec<f32> {
    let verts = mesh.face_verts(face);
    let at = |index: usize| mesh.positions[verts[index] as usize];
    let count = verts.len();
    let mut angles = vec![0.0; count];
    let mut this = count - 1;
    let mut previous = normalize(at(this - 1) - at(this));
    for next in 0..count {
        let following = normalize(at(this) - at(next));
        angles[this] = angle_normalized(previous, following);
        previous = following;
        this = next;
    }
    angles
}

fn mode_pairs(mesh: &Polymesh, mode: WeightedNormalMode) -> Vec<(f32, usize)> {
    let mut pairs: Vec<(f32, usize)> = match mode {
        WeightedNormalMode::FaceArea => (0..mesh.face_count())
            .map(|face| (face_area(mesh, face), face))
            .collect(),
        WeightedNormalMode::CornerAngle | WeightedNormalMode::FaceAreaAngle => {
            let mut pairs = Vec::with_capacity(mesh.corner_count());
            for face in 0..mesh.face_count() {
                let area = match mode {
                    WeightedNormalMode::FaceAreaAngle => face_area(mesh, face),
                    _ => 1.0,
                };
                let start = mesh.face(face).start;
                for (offset, angle) in face_angles(mesh, face).into_iter().enumerate() {
                    let value = PI - angle;
                    let value = match mode {
                        WeightedNormalMode::FaceAreaAngle => value * area,
                        _ => value,
                    };
                    pairs.push((value, start + offset));
                }
            }
            pairs
        }
    };
    pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
    pairs
}

fn group_members(settings: &WeightedNormal, mesh: &Polymesh) -> Option<Vec<bool>> {
    if settings.vertex_group.is_empty() || !mesh.vertex_groups.has_weights() {
        return None;
    }
    let group = mesh.vertex_groups.index(&settings.vertex_group)?;
    Some(
        mesh.vertex_groups
            .weights
            .iter()
            .map(|influences| influences.iter().any(|influence| influence.group == group))
            .collect(),
    )
}

pub(crate) fn evaluate(settings: &WeightedNormal, mesh: &mut Polymesh) {
    if mesh.face_count() == 0 {
        return;
    }
    let face_normals = mesh.face_normals();
    let strengths = match settings.use_face_influence {
        true => mesh
            .attributes
            .values::<i32>(FACE_STRENGTH, Domain::Face)
            .map(<[i32]>::to_vec),
        false => None,
    };
    let members = group_members(settings, mesh);
    let has_group = members.is_some();
    let corner_faces = mesh.corner_faces();
    let (item_of, item_count): (Vec<usize>, usize) = match settings.keep_sharp {
        true => {
            let fans = mesh.corner_fans();
            let mut item_of = vec![0; mesh.corner_count()];
            for (index, fan) in fans.iter().enumerate() {
                for &corner in fan {
                    item_of[corner as usize] = index;
                }
            }
            (item_of, fans.len())
        }
        false => (
            mesh.corner_verts
                .iter()
                .map(|&point| point as usize)
                .collect(),
            mesh.point_count(),
        ),
    };
    let start = Item {
        normal: Vec3::ZERO,
        loops: 0,
        value: 0.0,
        strength: FACE_STRENGTH_WEAK,
    };
    let mut items = vec![start; item_count];
    let mut weighting = Weighting {
        settings,
        weight: weight(settings.weight),
        face_normals: &face_normals,
        strengths: strengths.as_deref(),
        members,
        powers: [0.0; CACHED_POWERS],
    };
    let pairs = mode_pairs(mesh, settings.mode);
    match settings.mode {
        WeightedNormalMode::FaceArea => {
            for &(value, face) in &pairs {
                for corner in mesh.face(face) {
                    let point = mesh.corner_verts[corner] as usize;
                    weighting.aggregate(&mut items[item_of[corner]], point, face, value);
                }
            }
        }
        WeightedNormalMode::CornerAngle | WeightedNormalMode::FaceAreaAngle => {
            for &(value, corner) in &pairs {
                let point = mesh.corner_verts[corner] as usize;
                let face = corner_faces[corner] as usize;
                weighting.aggregate(&mut items[item_of[corner]], point, face, value);
            }
        }
    }
    for item in &mut items {
        let squared = item.normal.dot(item.normal);
        let (normal, length) = match squared > 1.0e-35 {
            true => {
                let length = squared.sqrt();
                (item.normal * (1.0 / length), length)
            }
            false => (Vec3::ZERO, 0.0),
        };
        item.normal = match length < VALID_LENGTH {
            true => Vec3::ZERO,
            false => normal,
        };
    }
    let targets: Vec<Vec3> = match settings.keep_sharp || has_group {
        true => {
            let current = mesh.corner_normals();
            (0..mesh.corner_count())
                .map(|corner| match items[item_of[corner]].normal {
                    Vec3::ZERO => current[corner],
                    normal => normal,
                })
                .collect()
        }
        false => (0..mesh.corner_count())
            .map(|corner| items[item_of[corner]].normal)
            .collect(),
    };
    mesh.sharp_edges_mut();
    let encoded = mesh.encode_custom_normals(&targets);
    let decoded = mesh.decode_custom_normals(&encoded);
    mesh.custom_normals_mut().copy_from_slice(&decoded);
}

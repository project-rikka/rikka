use animate_body::{Body, Expression, Faces, HumanBone, Side, Worlds};
use rikka_model::glam::Vec3;

use crate::easy::part::Marker;

const LOOK_AHEAD: f32 = 0.3;
const MOUTH_RISE: f32 = 0.35;
const MOUTH_AHEAD: f32 = 0.3;

pub(super) fn markers(body: &Body, faces: &Faces, worlds: &Worlds) -> Vec<(Marker, Vec3)> {
    let mut eyes = Side::BOTH
        .into_iter()
        .map(Side::eye)
        .filter(|eye| body.has(*eye))
        .peekable();
    if eyes.peek().is_none() || !body.has(HumanBone::Head) {
        return Vec::new();
    }
    let (count, middle, looking) = eyes.fold((0.0, Vec3::ZERO, Vec3::ZERO), |sum, eye| {
        (
            sum.0 + 1.0,
            sum.1 + worlds.position(eye),
            sum.2 + worlds.rotation(eye) * Vec3::Z,
        )
    });
    let middle = middle / count;
    let head = worlds.joint(HumanBone::Head);
    let looking = looking.normalize_or(head.rotation * Vec3::Z);
    let mut markers = vec![(Marker::Eyes, middle + looking * LOOK_AHEAD * body.scale())];
    let speaks = Expression::VOWELS.into_iter().any(|vowel| faces.has(vowel));
    if speaks {
        let local = head.rotation.inverse() * (middle - head.position);
        let mouth = Vec3::new(
            0.0,
            local.y * MOUTH_RISE,
            local.z + local.y.abs() * MOUTH_AHEAD,
        );
        markers.push((Marker::Mouth, head.position + head.rotation * mouth));
    }
    markers
}

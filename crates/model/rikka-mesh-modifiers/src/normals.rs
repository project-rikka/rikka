use glam::{Vec2, Vec3};
use rikka_polymesh::{Attribute, CUSTOM_NORMAL, Domain, Polymesh, Values, acos_approx};

const ENCODED_NORMAL: &str = ".rikka_encoded_normal";

pub(crate) fn encode_custom(mesh: &mut Polymesh) -> bool {
    let Some(normals) = mesh.custom_normals().map(<[Vec3]>::to_vec) else {
        return false;
    };
    let encoded = mesh.encode_custom_normals(&normals);
    mesh.attributes.remove(CUSTOM_NORMAL);
    mesh.attributes.insert(Attribute::new(
        ENCODED_NORMAL,
        Domain::Corner,
        Values::Vec2(
            encoded
                .iter()
                .map(|&[x, y]| Vec2::new(f32::from(x), f32::from(y)))
                .collect(),
        ),
    ));
    true
}

pub(crate) fn decode_custom(mesh: &mut Polymesh) {
    let Some(attribute) = mesh.attributes.remove(ENCODED_NORMAL) else {
        return;
    };
    let Values::Vec2(values) = attribute.values else {
        return;
    };
    let encoded: Vec<[i16; 2]> = values
        .iter()
        .map(|value| [value.x as i16, value.y as i16])
        .collect();
    let normals = mesh.decode_custom_normals(&encoded);
    mesh.custom_normals_mut().copy_from_slice(&normals);
}

pub(crate) fn shading_point_normals(mesh: &Polymesh) -> Vec<Vec3> {
    let Some(custom) = mesh.custom_normals() else {
        return mesh.point_normals();
    };
    let mut sums = vec![Vec3::ZERO; mesh.point_count()];
    let mut used = vec![false; mesh.point_count()];
    for face in 0..mesh.face_count() {
        for corner in mesh.face(face) {
            let point = mesh.corner_verts[corner] as usize;
            let position = mesh.positions[point];
            let previous =
                mesh.positions[mesh.corner_verts[mesh.previous_corner(face, corner)] as usize];
            let next = mesh.positions[mesh.corner_verts[mesh.next_corner(face, corner)] as usize];
            let factor = acos_approx(
                (previous - position)
                    .normalize_or_zero()
                    .dot((next - position).normalize_or_zero()),
            );
            sums[point] += custom[corner] * factor;
            used[point] = true;
        }
    }
    sums.iter()
        .zip(&used)
        .zip(&mesh.positions)
        .map(|((sum, &used), position)| match used {
            true => sum.normalize_or_zero(),
            false => position.normalize_or_zero(),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NormalsDomain {
    Point,
    Face,
    Corner,
}

pub(crate) fn normals_domain(mesh: &Polymesh) -> NormalsDomain {
    if mesh.face_count() == 0 {
        return NormalsDomain::Point;
    }
    if mesh.custom_normals().is_some() {
        return NormalsDomain::Corner;
    }
    let mix = |values: Option<&[bool]>| match values {
        None => (false, true),
        Some(values) => (
            values.iter().all(|&value| value),
            values.iter().all(|&value| !value),
        ),
    };
    let (faces_all, faces_none) = mix(mesh.sharp_faces());
    if faces_all {
        return NormalsDomain::Face;
    }
    let (edges_all, edges_none) = mix(mesh.sharp_edges());
    if edges_all && mesh.edge_count() > 0 {
        return NormalsDomain::Face;
    }
    match edges_none && faces_none {
        true => NormalsDomain::Point,
        false => NormalsDomain::Corner,
    }
}

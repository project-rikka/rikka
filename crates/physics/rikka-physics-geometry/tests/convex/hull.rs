use rikka_physics_core::{Blob, BlobCache};
use rikka_physics_math::Vec3;

use crate::support::{Random, face_rings};
use rikka_physics_geometry::{ConvexHull, Error};

fn assert_valid(hull: &ConvexHull, inputs: &[Vec3]) {
    let edges = hull.edges();
    let v = hull.vertex_count() as isize;
    let e = (edges.len() / 2) as isize;
    let f = hull.face_count() as isize;
    assert_eq!(v - e + f, 2);
    for (index, edge) in edges.iter().enumerate() {
        assert_eq!(edge.twin as usize, index ^ 1);
        assert_eq!(edges[edge.twin as usize].twin as usize, index);
        assert_eq!(
            edges[edge.next as usize].origin,
            edges[edge.twin as usize].origin
        );
    }
    let scale = hull.bounds().extent().length();
    for plane in hull.planes() {
        for &point in inputs {
            assert!(
                plane.separation(point) <= 1.0e-4 * scale,
                "{}",
                plane.separation(point)
            );
        }
    }
    for (face, ring) in face_rings(hull).into_iter().enumerate() {
        for edge in ring {
            let point = hull.points()[edges[edge].origin as usize];
            assert!(hull.planes()[face].separation(point).abs() <= 1.0e-4 * scale);
        }
    }
    assert!(hull.volume() > 0.0 && hull.inner_radius() > 0.0);
}

#[test]
fn random_clouds_build_valid_hulls() {
    let mut random = Random::new(21);
    for count in [4, 8, 20, 60, 200] {
        let points: Vec<Vec3> = (0..count).map(|_| random.vector(2.0)).collect();
        let hull = ConvexHull::new(&points).expect("hull");
        assert_valid(&hull, &points);
    }
}

#[test]
fn degenerate_inputs_fail() {
    let flat = [
        Vec3::ZERO,
        Vec3::X,
        Vec3::Z,
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(0.5, 0.0, 0.3),
    ];
    assert!(matches!(
        ConvexHull::new(&flat),
        Err(Error::DegenerateHull { .. })
    ));
    assert!(ConvexHull::new(&flat[..3]).is_err());
    assert!(matches!(
        ConvexHull::new(&[Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(f32::NAN, 0.0, 0.0)]),
        Err(Error::NonFinite { .. })
    ));
}

#[test]
fn blobs_round_trip_and_dedup() {
    let mut random = Random::new(17);
    let points: Vec<Vec3> = (0..40).map(|_| random.vector(1.0)).collect();
    let a = ConvexHull::new(&points).expect("hull");
    let b = ConvexHull::new(&points).expect("hull");
    assert_eq!(a.content_hash(), b.content_hash());
    let cache = BlobCache::new();
    let a = a.interned(&cache);
    let b = b.interned(&cache);
    assert!(a.blob().shares_storage(b.blob()));

    let copy =
        ConvexHull::from_blob(Blob::from_bytes(a.blob().bytes()).expect("bytes")).expect("valid");
    assert_eq!(copy, a);
    let mut corrupt = a.blob().bytes().to_vec();
    let edges = a.header().edges;
    corrupt[edges.offset as usize + 2] = 0xff;
    corrupt[edges.offset as usize + 3] = 0x7f;
    let corrupt = Blob::from_bytes(&corrupt).expect("bytes");
    assert!(ConvexHull::from_blob(corrupt).is_err());
}

#[test]
fn derived_sections_are_validated() {
    let mut random = Random::new(19);
    let points: Vec<Vec3> = (0..24).map(|_| random.vector(1.0)).collect();
    let hull = ConvexHull::new(&points).expect("hull");
    let header = *hull.header();
    for offset in [
        header.edge_normals.offset,
        header.face_distances.offset,
        header.soa_points.offset,
    ] {
        let mut bytes = hull.blob().bytes().to_vec();
        bytes[offset as usize + 1] ^= 0x40;
        let tampered = Blob::from_bytes(&bytes).expect("bytes");
        assert!(matches!(
            ConvexHull::from_blob(tampered),
            Err(Error::CorruptShape { .. })
        ));
    }
}

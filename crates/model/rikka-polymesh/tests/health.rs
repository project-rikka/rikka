use glam::Vec3;
use rikka_polymesh::Polymesh;

use crate::support::{CUBE_FACES, CUBE_POINTS, cube, points};

fn reversed(faces: &[usize]) -> Polymesh {
    let mut polygons = CUBE_FACES;
    for &face in faces {
        polygons[face].reverse();
    }
    Polymesh::from_polygons(points(&CUBE_POINTS), polygons)
}

#[test]
fn orientation_flags_faces_against_the_outside() {
    let inverted = {
        let mut mesh = cube();
        mesh.positions
            .iter_mut()
            .for_each(|point| point.x = -point.x);
        mesh
    };
    let open = Polymesh::from_polygons(
        vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::Y,
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
        ],
        vec![vec![0, 1, 2, 3], vec![1, 2, 5, 4]],
    );
    let cases: [(&str, Polymesh, Vec<bool>, usize); 4] = [
        ("clean", cube(), vec![false; 6], 0),
        (
            "one reversed",
            reversed(&[2]),
            vec![false, false, true, false, false, false],
            4,
        ),
        ("inverted", inverted, vec![true; 6], 0),
        ("open, larger side wins", open, vec![true, false], 1),
    ];
    for (name, mesh, flipped, inconsistent) in cases {
        let orientation = mesh.orientation();
        assert_eq!(orientation.flipped, flipped, "{name}");
        assert_eq!(
            orientation
                .inconsistent_edges
                .iter()
                .filter(|&&edge| edge)
                .count(),
            inconsistent,
            "{name}"
        );
    }
}

#[test]
fn degenerate_faces_and_self_intersections_name_faces() {
    let sliver = Polymesh::from_polygons(
        vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0, Vec3::Y],
        vec![vec![0, 1, 2], vec![0, 1, 3]],
    );
    assert_eq!(sliver.degenerate_faces(), [true, false]);
    let mut crossing = cube();
    crossing.append(&{
        let mut moved = cube();
        moved
            .positions
            .iter_mut()
            .for_each(|point| *point += Vec3::ONE);
        moved
    });
    let pairs = crossing.self_intersections();
    assert!(!pairs.is_empty());
    assert!(pairs.iter().all(|&[a, b]| (a < 6) != (b < 6)), "{pairs:?}");
    assert!(cube().self_intersections().is_empty());
}

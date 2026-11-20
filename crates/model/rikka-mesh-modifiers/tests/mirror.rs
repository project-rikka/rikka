use glam::Vec3;
use rikka_mesh_modifiers::Mirror;

use crate::oracle::basic::{MIRROR_BISECT_YZ, MIRROR_MERGE_UV_GROUPS, MIRROR_SEPARATE};
use crate::support::{
    CUBE, GRID, assert_edges, assert_matches, assert_uvs, fixture, moved, run, with_side_groups,
    with_uvs,
};

#[test]
fn mirror_copies_across_axis() {
    let cube = moved(fixture(&CUBE), Vec3::new(1.5, 0.0, 0.0), Vec3::ONE);
    let mirror = Mirror {
        use_mirror_merge: false,
        ..Mirror::default()
    };
    assert_matches(&run(&cube, mirror).mesh, &MIRROR_SEPARATE);
}

#[test]
fn mirror_merges_and_flips_uvs_and_groups() {
    let grid = with_side_groups(with_uvs(fixture(&GRID)));
    let mirror = Mirror {
        use_mirror_u: true,
        mirror_offset: glam::Vec2::new(0.25, 0.0),
        offset: glam::Vec2::new(0.0, 0.5),
        ..Mirror::default()
    };
    let evaluated = run(&grid, mirror).mesh;
    assert_matches(&evaluated, &MIRROR_MERGE_UV_GROUPS);
    assert_uvs(&evaluated, "UVMap", MIRROR_MERGE_UV_GROUPS.uvs);
    for (point, expected) in MIRROR_MERGE_UV_GROUPS.groups.iter().enumerate() {
        for &(group, weight) in expected.iter() {
            let actual = evaluated.vertex_groups.weight(point, group);
            assert!(
                (actual - weight).abs() < 1.0e-5,
                "point {point} group {group}: {actual} != {weight}"
            );
        }
        assert_eq!(
            evaluated.vertex_groups.weights[point].len(),
            expected.len(),
            "point {point}"
        );
    }
}

#[test]
fn mirror_bisects_and_mirrors_two_axes() {
    let cube = moved(fixture(&CUBE), Vec3::new(0.3, 0.25, 0.5), Vec3::ONE);
    let mirror = Mirror {
        use_axis: [false, true, true],
        use_bisect_axis: [false, true, false],
        use_bisect_flip_axis: [false, true, false],
        ..Mirror::default()
    };
    let evaluated = run(&cube, mirror).mesh;
    assert_matches(&evaluated, &MIRROR_BISECT_YZ);
    assert_edges(&evaluated, &MIRROR_BISECT_YZ);
}

mod fixture;

use std::collections::BTreeMap;

use glam::Vec3;
use rikka_spatial::PointTree;

use crate::support::Rng;

fn brute_sorted(points: &[Vec3], query: Vec3) -> Vec<(f32, u32)> {
    let mut distances: Vec<(f32, u32)> = points
        .iter()
        .enumerate()
        .map(|(index, point)| (point.distance(query), index as u32))
        .collect();
    distances.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
    distances
}

#[test]
fn nearest_queries_match_brute_force() {
    let mut rng = Rng::new(80);
    let mut points: Vec<Vec3> = (0..3_000).map(|_| rng.vec3(-1.0, 1.0)).collect();
    let copies: Vec<Vec3> = (0..200).map(|index| points[index * 7]).collect();
    points.extend(copies);
    let tree = PointTree::new(points.iter().copied());
    assert_eq!(tree.len(), points.len());
    for _ in 0..300 {
        let query = match rng.index(4) {
            0 => points[rng.index(points.len())],
            _ => rng.vec3(-1.3, 1.3),
        };
        let brute = brute_sorted(&points, query);
        let nearest = tree.nearest(query).unwrap();
        assert_eq!((nearest.index, nearest.distance), (brute[0].1, brute[0].0));
        let k = 1 + rng.index(20);
        let found: Vec<(f32, u32)> = tree
            .k_nearest(query, k)
            .iter()
            .map(|nearest| (nearest.distance, nearest.index))
            .collect();
        assert_eq!(found, brute[..k].to_vec());
        let radius = rng.range(0.0, 0.3);
        let within: Vec<u32> = tree
            .within_radius(query, radius)
            .iter()
            .map(|nearest| nearest.index)
            .collect();
        let expected: Vec<u32> = brute
            .iter()
            .map(|&(_, index)| index)
            .filter(|&index| points[index as usize].distance_squared(query) <= radius * radius)
            .collect();
        assert_eq!(within, expected);
        let filtered = tree
            .nearest_filtered(query, |index| index % 5 == 2)
            .unwrap();
        let expected = brute.iter().find(|(_, index)| index % 5 == 2).unwrap();
        assert_eq!((filtered.distance, filtered.index), *expected);
    }
    assert!(PointTree::new([]).nearest(Vec3::ZERO).is_none());
    assert!(tree.k_nearest(Vec3::ZERO, 0).is_empty());
}

#[test]
fn indexed_trees_report_caller_indices() {
    let tree = PointTree::from_indexed([(40, Vec3::ZERO), (7, Vec3::X), (12, Vec3::Y * 3.0)]);
    assert_eq!(tree.nearest(Vec3::new(0.9, 0.0, 0.0)).unwrap().index, 7);
    let duplicates =
        PointTree::from_indexed([(5, Vec3::ZERO), (2, Vec3::splat(0.001))]).find_duplicates(0.01);
    assert_eq!(duplicates.target(5), 2);
    assert_eq!(duplicates.target(2), 2);
    assert_eq!(duplicates.target(3), 3);
    assert_eq!(duplicates.merged_count(), 1);
}

fn greedy_reference(points: &[Vec3], distance: f32) -> Vec<u32> {
    let mut marks = vec![-1i64; points.len()];
    for index in 0..points.len() {
        if marks[index] != -1 && marks[index] != index as i64 {
            continue;
        }
        let mut found = false;
        for other in 0..points.len() {
            if other != index && marks[other] == -1 {
                let offset = points[other] - points[index];
                if offset.x * offset.x + offset.y * offset.y + offset.z * offset.z
                    <= distance * distance
                {
                    marks[other] = index as i64;
                    found = true;
                }
            }
        }
        if found {
            marks[index] = index as i64;
        }
    }
    marks
        .iter()
        .enumerate()
        .map(|(index, &mark)| if mark < 0 { index as u32 } else { mark as u32 })
        .collect()
}

#[test]
fn find_duplicates_follows_index_order_semantics() {
    let mut rng = Rng::new(81);
    for round in 0..20 {
        let mut points: Vec<Vec3> = (0..1_500).map(|_| rng.vec3(-1.0, 1.0)).collect();
        for index in 0..300 {
            let source = points[rng.index(points.len())];
            points[index * 3] = source + rng.vec3(-0.01, 0.01);
        }
        let distance = 0.01 + round as f32 * 0.002;
        let duplicates = PointTree::new(points.iter().copied()).find_duplicates(distance);
        assert_eq!(
            duplicates.targets(),
            greedy_reference(&points, distance).as_slice()
        );
        for (index, &target) in duplicates.targets().iter().enumerate() {
            assert_eq!(
                duplicates.target(target),
                target,
                "chains never form ({index})"
            );
        }
    }
}

fn fixture_points() -> Vec<Vec3> {
    fixture::POINTS
        .iter()
        .map(|point| Vec3::from_array(*point))
        .collect()
}

#[test]
fn find_duplicates_matches_blender_weld() {
    let points = fixture_points();
    let duplicates = PointTree::new(points.iter().copied()).find_duplicates(fixture::DISTANCE);
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (index, &target) in duplicates.targets().iter().enumerate() {
        groups.entry(target).or_default().push(index as u32);
    }
    let mut merged: Vec<Vec<u32>> = groups
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();
    merged.sort();
    let expected: Vec<Vec<u32>> = fixture::WELD_GROUPS
        .iter()
        .map(|group| group.to_vec())
        .collect();
    assert_eq!(merged, expected);
    for group in fixture::WELD_GROUPS {
        assert!(
            group
                .iter()
                .all(|&member| duplicates.target(member) == group[0])
        );
    }
}

#[test]
fn find_duplicate_clusters_matches_blender_remove_doubles() {
    let points = fixture_points();
    let tree = PointTree::new(points.iter().copied());
    for (keep, expected) in [
        (&[][..], &fixture::CLUSTER_MERGES[..]),
        (&fixture::KEEP[..], &fixture::KEPT_MERGES[..]),
    ] {
        let duplicates =
            tree.find_duplicate_clusters(fixture::DISTANCE, |index| keep.contains(&index));
        let merges: Vec<(u32, u32)> = duplicates
            .targets()
            .iter()
            .enumerate()
            .filter(|&(index, &target)| index as u32 != target)
            .map(|(index, &target)| (index as u32, target))
            .collect();
        assert_eq!(merges, expected);
        assert_eq!(duplicates.merged_count(), expected.len());
    }
}

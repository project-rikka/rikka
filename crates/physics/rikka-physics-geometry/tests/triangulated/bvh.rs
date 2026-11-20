use rikka_physics_core::WorkerPool;
use rikka_physics_math::{Aabb, Vec3};

use rikka_physics_geometry::*;

struct Random(u64);

impl Random {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn vector(&mut self, scale: f32) -> Vec3 {
        Vec3::new(self.next(), self.next(), self.next()) * scale
    }
}

fn boxes(count: usize, seed: u64) -> Vec<Aabb> {
    let mut random = Random(seed);
    (0..count)
        .map(|_| {
            let center = random.vector(100.0);
            let extent = random.vector(2.0) + Vec3::splat(0.01);
            Aabb::from_center_extent(center, extent)
        })
        .collect()
}

fn tree(bounds: &[Aabb]) -> Bvh4 {
    Bvh4::builder().build(bounds, 0..bounds.len() as u32)
}

fn sorted(mut values: Vec<u32>) -> Vec<u32> {
    values.sort_unstable();
    values
}

#[test]
fn overlap_queries_match_brute_force() {
    let bounds = boxes(3000, 1);
    let tree = tree(&bounds);
    let quantized = tree.quantize();
    let mut random = Random(2);
    for _ in 0..50 {
        let query = Aabb::from_center_extent(random.vector(100.0), random.vector(10.0));
        let expected: Vec<u32> = (0..bounds.len() as u32)
            .filter(|&item| bounds[item as usize].overlaps(&query))
            .collect();
        let mut found = Vec::new();
        tree.query(query, |item| {
            found.push(item);
            true
        });
        let found = sorted(found);
        assert!(
            expected
                .iter()
                .all(|item| found.binary_search(item).is_ok())
        );
        let exact: Vec<u32> = found
            .into_iter()
            .filter(|&item| bounds[item as usize].overlaps(&query))
            .collect();
        assert_eq!(exact, expected);
        let mut coarse = Vec::new();
        quantized.tree().query(query, |slot| {
            coarse.push(tree.items()[slot as usize]);
            true
        });
        let coarse = sorted(coarse);
        assert!(
            expected
                .iter()
                .all(|item| coarse.binary_search(item).is_ok())
        );
    }
}

#[test]
fn ray_cast_finds_nearest_box() {
    let bounds = boxes(2000, 3);
    let tree = tree(&bounds);
    let quantized = tree.quantize();
    let mut random = Random(4);
    for _ in 0..100 {
        let origin = random.vector(100.0) - Vec3::splat(20.0);
        let translation = random.vector(140.0) - Vec3::splat(70.0);
        let expected = bounds
            .iter()
            .filter_map(|b| b.ray_fraction(origin, translation, 1.0))
            .fold(1.0f32, f32::min);
        let fraction = tree.ray_cast(origin, translation, 1.0, |item, best| {
            bounds[item as usize]
                .ray_fraction(origin, translation, best)
                .unwrap_or(best)
        });
        assert!((fraction - expected).abs() < 1e-5, "{fraction} {expected}");
        let coarse = quantized
            .tree()
            .ray_cast(origin, translation, 1.0, |slot, best| {
                let item = tree.items()[slot as usize];
                bounds[item as usize]
                    .ray_fraction(origin, translation, best)
                    .unwrap_or(best)
            });
        assert!((coarse - expected).abs() < 1e-5);
    }
}

fn overlapping_pairs(bounds: &[Aabb], keep: impl Fn(u32, u32) -> bool) -> Vec<(u32, u32)> {
    let mut expected = Vec::new();
    for a in 0..bounds.len() as u32 {
        for b in a + 1..bounds.len() as u32 {
            if bounds[a as usize].overlaps(&bounds[b as usize]) && keep(a, b) {
                expected.push((a, b));
            }
        }
    }
    expected
}

fn normalized(mut pairs: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    for pair in &mut pairs {
        *pair = (pair.0.min(pair.1), pair.0.max(pair.1));
    }
    pairs.sort_unstable();
    pairs
}

fn task_pairs(tree: &Bvh4, tasks: &[PairTask]) -> Vec<(u32, u32)> {
    let mut found = Vec::new();
    for &task in tasks {
        tree.run_self_pair_task(task, |a, b| {
            found.push((a, b));
            true
        });
    }
    normalized(found)
}

#[test]
fn self_pairs_match_overlapping_pairs() {
    let bounds = boxes(1200, 7);
    let expected = overlapping_pairs(&bounds, |_, _| true);
    for leaf_size in [1, 4, 16] {
        let tree = Bvh4::builder()
            .leaf_size(leaf_size)
            .build(&bounds, 0..bounds.len() as u32);
        let mut found = Vec::new();
        tree.self_pairs(|a, b| {
            found.push((a, b));
            true
        });
        assert_eq!(normalized(found), expected);
        let mut tasks = Vec::new();
        tree.self_pair_tasks(64, &mut tasks);
        assert!(tasks.len() >= 64);
        assert_eq!(task_pairs(&tree, &tasks), expected);
    }
}

#[test]
fn parallel_refit_matches_serial_refit() {
    let mut bounds = boxes(9000, 31);
    let mut serial = tree(&bounds);
    let mut random = Random(32);
    let pool = WorkerPool::builder()
        .workers(4)
        .build()
        .expect("worker pool");
    for round in 0..6 {
        for b in bounds.iter_mut().skip(round * 7).step_by(5) {
            *b = b.translated(random.vector(6.0) - Vec3::splat(3.0));
        }
        for item in (round as u32 * 31..9000).step_by(997) {
            let location = serial.location(item).expect("placed");
            let root = serial
                .parent(location.node)
                .map_or(location.node, |at| at.node);
            serial.rebuild_subtree(root, &bounds);
        }
        let mut parallel = serial.clone();
        serial.refit(&bounds);
        parallel.refit_on(&bounds, &pool);
        assert_eq!(serial, parallel);
        for item in 0..bounds.len() as u32 {
            let location = serial.location(item).expect("every item is placed");
            assert!(
                serial
                    .node(location.node)
                    .bounds(location.slot as usize)
                    .contains(&bounds[item as usize])
            );
        }
        for (index, node) in serial.nodes().iter().enumerate() {
            for slot in 0..4 {
                if let Child::Node(child) = node.child(slot) {
                    assert!(child as usize > index);
                }
            }
        }
    }
}

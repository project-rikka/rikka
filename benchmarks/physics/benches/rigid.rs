use std::sync::LazyLock;

use divan::Bencher;
use divan::counter::ItemsCount;
use physics_bench::{Box3d, Bullet, Engine, Jolt, Rapier, Rikka, Scene, Size, Stage};

static SIZE: LazyLock<Size> = LazyLock::new(Size::from_args);

fn main() {
    divan::main();
}

fn worker_counts() -> Vec<usize> {
    SIZE.worker_counts()
}

fn simulate<E: Engine>(bencher: Bencher, scene: &Scene, workers: usize) {
    let timed = scene.steps(*SIZE) - 1;
    bencher
        .counter(ItemsCount::new(timed))
        .with_inputs(|| {
            let mut stage = Stage::new::<E>(scene, *SIZE, workers).expect("scene should build");
            stage.step().expect("first step should run");
            stage
        })
        .bench_local_values(|mut stage| {
            stage.run(timed).expect("scene should step");
            stage
        });
}

macro_rules! scenes {
    ($($scene:ident => $constant:ident),* $(,)?) => {
        $(
            mod $scene {
                use super::*;

                #[divan::bench(
                    types = [Rikka, Box3d, Jolt, Rapier],
                    args = worker_counts(),
                    sample_count = 5,
                    sample_size = 1,
                )]
                fn threaded<E: Engine>(bencher: Bencher, workers: usize) {
                    simulate::<E>(bencher, &physics_bench::$constant, workers);
                }

                #[divan::bench(sample_count = 5, sample_size = 1)]
                fn bullet(bencher: Bencher) {
                    simulate::<Bullet>(bencher, &physics_bench::$constant, 1);
                }
            }
        )*
    };
}

scenes! {
    convex_pile => CONVEX_PILE,
    joint_grid => JOINT_GRID,
    junkyard => JUNKYARD,
    large_pyramid => LARGE_PYRAMID,
    large_world => LARGE_WORLD,
    many_pyramids => MANY_PYRAMIDS,
    spinner => SPINNER,
    trees100 => TREES100,
    trees50 => TREES50,
    trees25 => TREES25,
    washer => WASHER,
}

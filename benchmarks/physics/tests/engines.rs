use glam::Vec3;
use physics_bench::{
    Blueprint, BodyDef, BodyIndex, Box3d, Bullet, Engine, GRAVITY, Geometry, INTERNALS, JOINT_GRID,
    Jolt, LARGE_PYRAMID, MANY_PYRAMIDS, Rapier, Rikka, SCENES, Setup, Size, Stage, TIME_STEP,
};

fn builds_and_steps<E: Engine>() {
    for scene in SCENES {
        let mut stage = Stage::new::<E>(scene, Size::Smoke, 2)
            .unwrap_or_else(|error| panic!("{} builds {}: {error}", E::NAME, scene.name));
        stage
            .run(3)
            .unwrap_or_else(|error| panic!("{} steps {}: {error}", E::NAME, scene.name));
        let last = BodyIndex(stage.bodies() as u32 - 1);
        let pose = stage.pose(last);
        assert!(
            pose.position.is_finite() && pose.rotation.is_finite(),
            "{} keeps {} finite",
            E::NAME,
            scene.name
        );
    }
}

fn falls_under_gravity<E: Engine>() {
    let mut blueprint = Blueprint::new(true);
    let ball = blueprint.add(
        BodyDef::dynamic()
            .position(Vec3::new(0.0, 100.0, 0.0))
            .shape(Geometry::sphere(0.5)),
    );
    let mut stage = Stage::from_setup::<E>(Setup::new(blueprint), 1).expect("ball should build");
    let steps = 30;
    stage.run(steps).expect("ball should fall");
    let elapsed = steps as f32 * TIME_STEP;
    let expected = 100.0 + 0.5 * GRAVITY.y * elapsed * elapsed;
    let fallen = stage.pose(ball).position.y;
    assert!(
        (fallen - expected).abs() < 0.1,
        "{} dropped the ball to {fallen}, not {expected}",
        E::NAME
    );
}

fn pyramid_stands<E: Engine>() {
    let mut stage = Stage::new::<E>(&LARGE_PYRAMID, Size::Smoke, 2).expect("pyramid should build");
    let top = BodyIndex(stage.bodies() as u32 - 1);
    let start = stage.pose(top).position;
    stage.run(60).expect("pyramid should step");
    let resting = stage.pose(top).position;
    assert!(
        resting.y > 0.9 * start.y && (resting.x - start.x).abs() < 0.5,
        "{} let the pyramid top fall from {start} to {resting}",
        E::NAME
    );
}

fn sleeps_at_rest<E: Engine>() {
    let mut blueprint = Blueprint::new(true);
    blueprint.add(
        BodyDef::fixed()
            .position(Vec3::new(0.0, -1.0, 0.0))
            .shape(Geometry::cuboid(Vec3::new(10.0, 1.0, 10.0))),
    );
    blueprint.add(
        BodyDef::dynamic()
            .position(Vec3::new(0.0, 0.5, 0.0))
            .shape(Geometry::cuboid(Vec3::splat(0.5))),
    );
    let mut stage = Stage::from_setup::<E>(Setup::new(blueprint), 1).expect("box should build");
    stage.run(20).expect("box should settle");
    assert_eq!(
        stage.awake_bodies(),
        1,
        "{} slept before the time to sleep",
        E::NAME
    );
    stage.run(100).expect("box should rest");
    assert_eq!(
        stage.awake_bodies(),
        0,
        "{} kept a resting box awake",
        E::NAME
    );
}

fn agrees<E: Engine>() {
    builds_and_steps::<E>();
    falls_under_gravity::<E>();
    pyramid_stands::<E>();
    sleeps_at_rest::<E>();
}

#[test]
fn engines_agree_on_every_scene() {
    std::thread::scope(|scope| {
        let engines = [
            scope.spawn(agrees::<Rikka>),
            scope.spawn(agrees::<Box3d>),
            scope.spawn(agrees::<Jolt>),
            scope.spawn(agrees::<Rapier>),
            scope.spawn(agrees::<Bullet>),
        ];
        for engine in engines {
            engine.join().expect("engine thread should finish");
        }
    });
}

#[test]
fn rikka_is_deterministic_across_worker_counts() {
    for scene in [&JOINT_GRID, &MANY_PYRAMIDS] {
        let hash = |workers| {
            let mut stage =
                Stage::new::<Rikka>(scene, Size::Smoke, workers).expect("scene should build");
            stage.run(5).expect("scene should step");
            stage.state_hash().expect("rikka should hash its state")
        };
        assert_eq!(hash(1), hash(3), "{} diverged across workers", scene.name);
    }
    for internal in INTERNALS {
        let checksum = |workers| {
            let mut workload = internal.create(workers).expect("workload should build");
            for _ in 0..3 {
                workload.step();
            }
            workload.checksum()
        };
        assert_eq!(
            checksum(1),
            checksum(3),
            "{} diverged across workers",
            internal.name
        );
    }
}

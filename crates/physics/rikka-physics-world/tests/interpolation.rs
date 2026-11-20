use rikka_physics_world::{Body, Sphere, Vec3, World};

#[test]
fn interpolated_poses_lag_one_tick() {
    let mut world = World::builder().build().unwrap();
    let ball = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 10.0, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
    let frame = 1.0 / 1000.0;
    let mut heights = vec![world.pose(ball).unwrap().p.y];
    for _ in 0..500 {
        world.advance(frame);
        let shown = world.pose(ball).unwrap().p.y;
        let latest = world.latest_pose(ball).unwrap().p.y;
        assert!(
            shown >= latest,
            "the display never runs ahead: {shown} < {latest}"
        );
        heights.push(shown);
    }
    let steps: Vec<f32> = heights.windows(2).map(|pair| pair[0] - pair[1]).collect();
    assert!(steps.iter().all(|&step| step >= 0.0), "the ball only falls");
    let tick_speed_change = 9.81 / 120.0 * frame as f32;
    let roughest = steps
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f32::max);
    assert!(
        roughest < 2.0 * tick_speed_change,
        "no jumps at tick boundaries: {roughest}"
    );
}

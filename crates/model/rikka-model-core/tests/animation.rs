use std::sync::Arc;

use rikka_model_core::glam::{Mat4, Vec3};
use rikka_model_core::{
    Animation, AnimationPlayer, Channel, Curve, Instance, Interpolation, Model, ModelBuilder, Node,
    Pose, Repeat, Skin, SkinId, Track, Transform, Weights,
};

use crate::support::triangle;

#[test]
fn curve_clamps_and_interpolates() {
    let linear = Curve::linear(vec![1.0, 2.0, 3.0], vec![Vec3::ZERO, Vec3::X, Vec3::Y]);
    assert_eq!(linear.sample(0.0), Some(Vec3::ZERO));
    assert_eq!(linear.sample(9.0), Some(Vec3::Y));
    assert_eq!(linear.duration(), 3.0);
    assert_eq!(linear.sample(1.5), Some(Vec3::new(0.5, 0.0, 0.0)));
    assert_eq!(linear.sample(2.5), Some(Vec3::new(0.5, 0.5, 0.0)));
    assert_eq!(linear.sample(f32::NAN), Some(Vec3::ZERO));

    let step = Curve {
        times: vec![0.0, 1.0],
        values: vec![Vec3::ZERO, Vec3::X],
        interpolation: Interpolation::Step,
    };
    assert_eq!(step.sample(0.99), Some(Vec3::ZERO));
    assert_eq!(step.sample(1.0), Some(Vec3::X));

    assert_eq!(Curve::<Vec3>::default().sample(0.0), None);
}

#[test]
fn cubic_spline_hits_keyframes() {
    let cubic = Curve {
        times: vec![0.0, 1.0],
        values: vec![
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::X,
            Vec3::ZERO,
        ],
        interpolation: Interpolation::CubicSpline,
    };
    assert_eq!(cubic.sample(0.0), Some(Vec3::ZERO));
    assert_eq!(cubic.sample(1.0), Some(Vec3::X));
    let middle = cubic.sample(0.5).expect("a value");
    assert!(middle.x > 0.4 && middle.x < 0.6, "{middle}");

    let weights = Weights {
        times: vec![0.0, 1.0],
        values: vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        count: 1,
        interpolation: Interpolation::CubicSpline,
    };
    let mut out = [0.0];
    assert!(weights.sample_into(1.0, &mut out));
    assert_eq!(out, [1.0]);
    assert!(weights.sample_into(0.5, &mut out));
    assert!(out[0] > 0.4 && out[0] < 0.6, "{out:?}");

    let linear = Weights {
        times: vec![0.0, 1.0],
        values: vec![0.0, 1.0, 1.0, 0.0],
        count: 2,
        interpolation: Interpolation::Linear,
    };
    let mut both = [0.0; 2];
    assert!(linear.sample_into(0.5, &mut both));
    assert_eq!(both, [0.5, 0.5]);
}

fn sliding() -> Model {
    let mut builder = ModelBuilder::new();
    let node = builder.add_node(Node::new("bone"));
    builder.add_animation(Animation {
        name: "slide".into(),
        channels: vec![Channel {
            target: node,
            track: Track::Translation(Curve::linear(
                vec![0.0, 2.0],
                vec![Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0)],
            )),
        }],
    });
    builder.finish().expect("one animated node is a model")
}

#[test]
fn player_loops_or_stops_at_end() {
    let model = sliding();
    let slide = model.find_animation("slide").expect("the animation");
    let bone = model.find_node("bone").expect("the bone");
    let mut player = AnimationPlayer::new();

    let mut pose = Pose::rest(&model);
    player.apply(&model, &mut pose);
    assert_eq!(pose.local(bone).translation, Vec3::ZERO);

    player.play(&model, slide);
    player.advance(1.0);
    player.apply(&model, &mut pose);
    assert_eq!(pose.local(bone).translation, Vec3::new(2.0, 0.0, 0.0));

    player.advance(1.5);
    assert!((player.time() - 0.5).abs() < 1e-6);
    assert!(player.is_playing());

    player.set_repeat(Repeat::Once);
    player.advance(9.0);
    assert_eq!(player.time(), 2.0);
    assert!(!player.is_playing());
}

fn skinned() -> Arc<Model> {
    let mut builder = ModelBuilder::new();
    let mesh = builder.add_mesh(triangle());
    let root = builder.add_node(Node::new("root").mesh(mesh));
    let bone = builder.add_node(Node::new("bone").transform(Transform::from_translation(Vec3::Y)));
    builder.attach(root, bone);
    let skin = builder.add_skin(Skin {
        joints: vec![bone],
        inverse_bind: vec![Mat4::from_translation(-Vec3::Y)],
        ..Skin::new("skin")
    });
    builder.model_mut().nodes[root.index()].skin = Some(skin);
    builder.add_animation(Animation {
        name: "lift".into(),
        channels: vec![Channel {
            target: bone,
            track: Track::Translation(Curve::linear(
                vec![0.0, 1.0],
                vec![Vec3::Y, Vec3::new(0.0, 3.0, 0.0)],
            )),
        }],
    });
    Arc::new(builder.finish().expect("a skinned model"))
}

#[test]
fn instance_animates_independently_of_clones() {
    let model = skinned();
    let bone = model.find_node("bone").expect("the bone");
    let lift = model.find_animation("lift").expect("the animation");
    let mut instance = Instance::new(Arc::clone(&model));
    assert_eq!(instance.world(bone).w_axis.truncate(), Vec3::Y);
    assert_eq!(instance.joints(SkinId(0))[0], Mat4::IDENTITY);

    instance.play(lift);
    let snapshot = instance.clone();
    instance.advance(0.5);
    assert_eq!(
        instance.world(bone).w_axis.truncate(),
        Vec3::new(0.0, 2.0, 0.0)
    );
    assert_eq!(
        instance.joints(SkinId(0))[0].w_axis.truncate(),
        Vec3::new(0.0, 1.0, 0.0)
    );
    assert_eq!(snapshot.world(bone).w_axis.truncate(), Vec3::Y);

    instance.seek(0.75);
    assert_eq!(
        instance.world(bone).w_axis.truncate(),
        Vec3::new(0.0, 2.5, 0.0)
    );
}

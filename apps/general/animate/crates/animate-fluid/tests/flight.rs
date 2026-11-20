use animate_fluid::{Aim, Anchor, EmitterLevels, EmitterState, Shape, Spot, Throw};
use rikka_model_core::glam::{Mat4, Quat, Vec3};

const EARTH: Vec3 = Vec3::new(0.0, -9.81, 0.0);

fn state(shape: Shape, aim: Aim) -> EmitterState {
    EmitterState {
        on: true,
        shape,
        aim,
        launch: Spot {
            anchor: Anchor::bone("miku", "右手首"),
            offset: Vec3::new(0.0, 0.0, 0.1),
        },
        target: Spot {
            anchor: Anchor::World,
            offset: Vec3::new(1.0, 0.0, 0.0),
        },
        levels: EmitterLevels::default(),
    }
}

fn hand(figure: &str, bone: &str) -> Option<Mat4> {
    (figure == "miku" && bone == "右手首").then(|| Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)))
}

#[test]
fn stream_flies_from_bone_to_target() {
    let flight = state(Shape::Stream, Aim::Ballistic)
        .flight(EARTH, hand)
        .expect("the hand is found");
    assert_eq!(flight.from, Vec3::new(0.0, 1.0, 0.1));
    let path = flight.path(EARTH, 16);
    assert_eq!(path.len(), 16);
    assert!(path[15].distance(Vec3::new(1.0, 0.0, 0.0)) < 1e-4);
    let weightless = state(Shape::Shower, Aim::Ballistic);
    let throw = weightless.throw(Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0), Vec3::ZERO);
    assert_eq!(throw.velocity, Vec3::new(weightless.levels.speed, 0.0, 0.0));
}

#[test]
fn drips_start_at_rest() {
    let drip = state(Shape::Drip, Aim::Direction);
    assert_eq!(drip.throw(Vec3::ZERO, Vec3::X, EARTH), Throw::STILL);
    let flight = drip.flight(EARTH, hand).expect("a drip point");
    assert_eq!(flight.to, flight.from);
    assert_eq!(
        state(Shape::surface("miku", [0]), Aim::Ballistic).flight(EARTH, hand),
        None
    );
    assert_eq!(
        state(Shape::Stream, Aim::Ballistic).flight(EARTH, |_, _| None),
        None
    );
}

#[test]
fn bone_anchor_offset_round_trip() {
    let wrist = Mat4::from_rotation_translation(
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        Vec3::new(0.3, 1.1, 0.2),
    );
    let anchor = Anchor::bone("miku", "右手首");
    let locate = |figure: &str, bone: &str| (figure == "miku" && bone == "右手首").then_some(wrist);
    let offset = Vec3::new(0.0, 0.0, 0.05);
    let world = anchor.world(offset, locate).expect("the hand is found");
    assert!(world.distance(Vec3::new(0.35, 1.1, 0.2)) < 1e-5, "{world}");
    let back = anchor.offset(world, locate).expect("and inverted");
    assert!(back.distance(offset) < 1e-5);
    assert_eq!(anchor.world(offset, |_, _| None), None);
}

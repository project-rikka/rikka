use crate::support::{Studio, apply, engine, pixel_at, pixels, request, square};
use cut_project::{
    ClipId, Direction, Edit, EditPoint, EffectSlot, Item, Project, Region, Transition,
    TransitionKind, TransitionKnob,
};
use rikka_effects::Kind;
use rikka_types::Color;

const SCALE: f32 = 0.125;
const WIDTH: u32 = 240;
const LENGTH: i64 = 10;

fn put(project: &Project, item: impl Into<Item>, at: i64, layer: usize) -> (Project, ClipId) {
    let (project, applied) = project
        .apply(&Edit::Insert {
            item: item.into(),
            at,
            length: 20,
            layer,
        })
        .expect("inserting");
    (project, applied.selected.expect("the inserted item")[0])
}

fn with_transition(transition: Transition) -> Project {
    let (project, _) = put(
        &Project::new(),
        square(4000.0, Color::rgb(1.0, 0.0, 0.0)),
        0,
        0,
    );
    let (project, blue) = put(&project, square(4000.0, Color::rgb(0.0, 0.0, 1.0)), 20, 0);
    apply(
        &project,
        Edit::Transitions {
            edit_points: vec![EditPoint::head(blue)],
            transition: Some(transition.with_length(LENGTH)),
        },
    )
}

fn transition(kind: TransitionKind) -> Transition {
    Transition::new(kind, Project::new().rate())
}

fn means(studio: &mut Studio, project: &Project, frame: i64) -> (f64, f64) {
    let composite = studio.compose(&request(project.clone(), frame, SCALE));
    let rgba = pixels(&composite.picture);
    let count = (rgba.len() / 4) as f64;
    let mean = |channel: usize| {
        rgba.chunks(4)
            .map(|pixel| f64::from(pixel[channel]))
            .sum::<f64>()
            / count
    };
    (mean(0), mean(2))
}

#[test]
fn cross_dissolve_is_halfway_at_cut() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let project = with_transition(transition(TransitionKind::CrossDissolve));

    let (red, blue) = means(&mut studio, &project, 20);

    assert!((100.0..160.0).contains(&red), "{red}");
    assert!((100.0..160.0).contains(&blue), "{blue}");
    assert!((red + blue - 255.0).abs() < 8.0, "{red} + {blue}");
}

#[test]
fn wipe_reveals_incoming_clip_behind_edge() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let hard = transition(TransitionKind::Wipe);
    let hard = Transition {
        direction: Direction::Right,
        tuning: hard
            .tuning
            .with(TransitionKind::Wipe, TransitionKnob::Softness, 0.0),
        ..hard
    };
    let project = with_transition(hard);

    let composite = studio.compose(&request(project, 20, SCALE));
    let rgba = pixels(&composite.picture);
    let left = pixel_at(&rgba, WIDTH, 10, 60);
    let right = pixel_at(&rgba, WIDTH, WIDTH - 10, 60);
    assert!(left[2] > 240 && left[0] < 15, "{left:?}");
    assert!(right[0] > 240 && right[2] < 15, "{right:?}");
}

#[test]
fn every_transition_goes_from_outgoing_to_incoming() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    for kind in TransitionKind::ALL {
        let project = with_transition(transition(kind));
        let span = project
            .transition_span_at(0, 20)
            .map(|transition_span| transition_span.span)
            .expect("a passage");
        assert_eq!(span, 15..25, "{kind:?}");

        let (red, blue) = means(&mut studio, &project, span.start);
        assert!(
            red > blue,
            "{kind:?} starts on the outgoing clip: {red} {blue}"
        );
        means(&mut studio, &project, 20);
        let (red, blue) = means(&mut studio, &project, span.end - 1);
        assert!(
            blue > red,
            "{kind:?} ends on the incoming clip: {red} {blue}"
        );
    }
}

#[test]
fn transition_without_outgoing_fades_in() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let (project, _) = put(
        &Project::new(),
        square(4000.0, Color::rgb(0.0, 1.0, 0.0)),
        0,
        0,
    );
    let (project, red) = put(&project, square(4000.0, Color::rgb(1.0, 0.0, 0.0)), 0, 1);
    let project = apply(
        &project,
        Edit::Transitions {
            edit_points: vec![EditPoint::head(red)],
            transition: Some(transition(TransitionKind::CrossDissolve).with_length(LENGTH)),
        },
    );

    let composite = studio.compose(&request(project, 5, SCALE));
    let middle = pixel_at(&pixels(&composite.picture), WIDTH, WIDTH / 2, 60);
    assert!((100..160).contains(&middle[0]), "{middle:?}");
    assert!((100..160).contains(&middle[1]), "{middle:?}");
}

#[test]
fn effect_region_fades_in_through_transition() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let (project, _) = put(
        &Project::new(),
        square(4000.0, Color::rgb(1.0, 0.0, 0.0)),
        0,
        0,
    );
    let (project, range) = put(&project, Region::whole(), 0, 1);
    let project = apply(
        &project,
        Edit::Effects {
            clip: range,
            effects: vec![EffectSlot::new(Kind::BlackWhite.default_effect())],
        },
    );
    let project = apply(
        &project,
        Edit::Transitions {
            edit_points: vec![EditPoint::head(range)],
            transition: Some(transition(TransitionKind::CrossDissolve).with_length(LENGTH)),
        },
    );
    let mut centre = |frame: i64| {
        let composite = studio.compose(&request(project.clone(), frame, SCALE));
        pixel_at(&pixels(&composite.picture), WIDTH, WIDTH / 2, 60)
    };

    let start = centre(0);
    let middle = centre(5);
    let after = centre(15);
    assert!(start[0] > 240 && start[1] < 15, "{start:?}");
    assert!(
        after[0].abs_diff(after[1]) < 6 && after[1] > 30,
        "{after:?}"
    );
    assert!(
        middle[0] > after[0]
            && middle[0] < start[0]
            && middle[1] > start[1]
            && middle[1] < after[1],
        "{start:?} {middle:?} {after:?}"
    );
}

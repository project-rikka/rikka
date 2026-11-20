use crate::support::{Oracle, Studio, Tally, engine, insert, pixel_at, pixels, request, square};
use cut_project::{Camera, ClipId, Graphic, Placement, Project, Viewpoint};
use rikka_types::{Color, Point, Rect};

const SCALE: f32 = 0.25;
const FRAME: i64 = 10;

fn card(side: f32, placement: Placement) -> Graphic {
    square(side, Color::rgb(1.0, 0.0, 0.0)).with_placement(placement)
}

fn red(pixel: [u8; 4]) -> bool {
    pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40
}

fn black(pixel: [u8; 4]) -> bool {
    pixel[0] < 40 && pixel[1] < 40 && pixel[2] < 40
}

fn render_tally(
    studio: &mut Studio,
    project: &Project,
    clip: ClipId,
    view: Viewpoint,
    side: f32,
) -> (Tally, Vec<u8>) {
    let composite = studio.compose(&request(project.clone(), FRAME, SCALE).viewpoint(view));
    let (width, height) = composite.picture.dimensions();
    let rgba = pixels(&composite.picture);
    let local = Rect::new(-side / 2.0, -side / 2.0, side, side);
    let oracle = Oracle::new(project, clip, FRAME, view, local, SCALE);
    (oracle.tally(&rgba, width, height, red, black), rgba)
}

fn faithful(tally: &Tally) {
    assert!(tally.inside > 50, "too little is inside: {tally:?}");
    assert!(tally.outside > 50, "too little is outside: {tally:?}");
    assert!(
        tally.wrong.len() <= 6,
        "{} pixels disagree with the projection, first {:?}",
        tally.wrong.len(),
        &tally.wrong[..tally.wrong.len().min(8)]
    );
}

fn height_of_red(rgba: &[u8], x: u32) -> usize {
    (0..270).filter(|y| red(pixel_at(rgba, 480, x, *y))).count()
}

#[test]
fn tilted_layer_renders_in_perspective() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let tilted = card(
        400.0,
        Placement {
            tilt_y: 50.0,
            three_d: true,
            ..Placement::default()
        },
    );
    let (project, clip) = insert(&Project::new(), tilted, 0);

    let (tally, rgba) = render_tally(&mut studio, &project, clip, Viewpoint::Output, 400.0);
    faithful(&tally);
    assert!(black(pixel_at(&rgba, 480, 240 + 45, 135)));
    let (left, right) = (
        height_of_red(&rgba, 240 - 25),
        height_of_red(&rgba, 240 + 25),
    );
    assert!(left.abs_diff(right) >= 4, "edges of {left} and {right} px");
}

#[test]
fn camera_pans_zooms_and_rolls_layers_below() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let (project, below) = insert(&Project::new(), card(400.0, Placement::default()), 0);
    let camera = Camera {
        interest: Point::new(100.0, 0.0),
        zoom: 1.5,
        roll: 30.0,
        pan: 10.0,
        ..Camera::default()
    };
    let (project, _) = insert(&project, camera, 1);

    let (tally, _) = render_tally(&mut studio, &project, below, Viewpoint::Output, 400.0);
    faithful(&tally);
    assert!(
        tally.inside > 15_000,
        "the camera zooms in: {}",
        tally.inside
    );
}

#[test]
fn item_behind_camera_is_clipped() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let crossing = card(
        1000.0,
        Placement {
            x: 200.0,
            depth: -2300.0,
            tilt_y: 75.0,
            three_d: true,
            ..Placement::default()
        },
    );
    let (project, clip) = insert(&Project::new(), crossing, 0);

    let (tally, rgba) = render_tally(&mut studio, &project, clip, Viewpoint::Output, 1000.0);
    faithful(&tally);
    assert!(black(pixel_at(&rgba, 480, 20, 135)));
    assert!(red(pixel_at(&rgba, 480, 470, 135)));
}

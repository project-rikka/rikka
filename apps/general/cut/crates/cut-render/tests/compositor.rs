use crate::support::{
    Studio, apply, band, effect, encode_test_video, engine, framed_project, insert, is_near, level,
    pixel_at, pixels, place, request, square,
};
use cut_project::{Appearance, Edit, EffectSlot, Placement, Project, Redraw, Region};
use rikka_effects::Kind;
use rikka_types::Color;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn red() -> Color {
    Color::rgb(1.0, 0.0, 0.0)
}

fn grey(pixel: [u8; 4]) -> bool {
    pixel[0].abs_diff(pixel[1]) <= 3 && pixel[1].abs_diff(pixel[2]) <= 3 && pixel[0] > 20
}

fn with_effects(project: &Project, clip: cut_project::ClipId, slots: Vec<EffectSlot>) -> Project {
    apply(
        project,
        Edit::Effects {
            clip,
            effects: slots,
        },
    )
}

#[test]
fn graphics_render_on_black_at_preview_scale() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let (project, _) = insert(&Project::new(), square(400.0, red()), 0);

    for (scale, size) in [(0.25, (480, 270)), (0.5, (960, 540))] {
        let composite = studio.compose(&request(project.clone(), 0, scale));
        assert_eq!(composite.picture.dimensions(), size);
        let rgba = pixels(&composite.picture);
        let (centre_x, centre_y) = (size.0 / 2, size.1 / 2);
        assert!(is_near(pixel_at(&rgba, size.0, centre_x, centre_y), RED, 2));
        assert!(is_near(pixel_at(&rgba, size.0, 4, 4), BLACK, 0));
        let edge = (100.0 * scale * 4.0) as u32 / 2;
        assert!(is_near(
            pixel_at(&rgba, size.0, centre_x - edge + 2, centre_y),
            RED,
            2
        ));
        assert!(is_near(
            pixel_at(&rgba, size.0, centre_x - edge - 3, centre_y),
            BLACK,
            0
        ));
        assert!(composite.exact);
    }
}

#[test]
fn region_affects_only_layers_below_inside_shape() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let (project, _) = insert(&Project::new(), square(800.0, red()), 0);
    let left = band(400.0, 800.0).with_placement(Placement {
        x: -200.0,
        ..Placement::default()
    });
    let (project, range) = insert(&project, left, 1);
    let project = with_effects(
        &project,
        range,
        vec![EffectSlot::new(Kind::BlackWhite.default_effect())],
    );

    let composite = studio.compose(&request(project.clone(), 0, 0.25));
    let rgba = pixels(&composite.picture);
    assert!(
        grey(pixel_at(&rgba, 480, 175, 135)),
        "inside the range turns grey"
    );
    assert!(
        is_near(pixel_at(&rgba, 480, 300, 135), RED, 2),
        "outside keeps its colour"
    );

    let inverted = Region {
        inverted: true,
        ..*project
            .locate(range)
            .expect("the range")
            .1
            .region()
            .expect("a region")
            .as_ref()
    };
    let project = apply(
        &project,
        Edit::Redraw {
            changes: vec![(range, Appearance::Region(inverted))],
            by: Redraw::Style,
            at: 0,
        },
    );
    let composite = studio.compose(&request(project, 0, 0.25));
    let rgba = pixels(&composite.picture);
    assert!(is_near(pixel_at(&rgba, 480, 175, 135), RED, 2));
    assert!(grey(pixel_at(&rgba, 480, 300, 135)));
}

#[test]
fn unchanged_frame_is_not_rerendered() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let (project, framed) = insert(&Project::new(), square(400.0, red()), 0);
    let project = with_effects(
        &project,
        framed,
        vec![EffectSlot::new(effect(
            Kind::GaussianBlur,
            &[("radius", 8.0)],
        ))],
    );

    let first = studio.compose(&request(project.clone(), 0, 0.25));
    let later = studio.compose(&request(project.clone(), 30, 0.25));
    assert!(first.picture == later.picture);

    let finer = studio.compose(&request(project, 30, 0.5));
    assert!(finer.picture != later.picture);
}

#[test]
fn footage_shows_playhead_frame_with_effects() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let directory = tempfile::tempdir().expect("a folder");
    let Some(media) = encode_test_video(&directory.path().join("greys.mkv"), 64, 48, 10, 40) else {
        return;
    };
    let (project, clip) = place(&framed_project(), media, 1);

    for (frame, index) in [(30, 10), (0, 0), (45, 15)] {
        let composite = studio.compose(&request(project.clone(), frame, 0.25));
        let centre = pixel_at(&pixels(&composite.picture), 480, 240, 135);
        let expected_level = level(index);
        assert!(
            is_near(
                centre,
                [expected_level, expected_level, expected_level, 255],
                4
            ),
            "frame {frame} shows film frame {index}: {centre:?}"
        );
        assert!(composite.exact);
    }

    let smaller = cut_project::Treatment {
        placement: Placement {
            zoom: 0.5,
            ..Placement::default()
        },
        ..*project
            .locate(clip)
            .expect("the clip")
            .1
            .treatment()
            .expect("footage")
    };
    let project = apply(
        &project,
        Edit::Redraw {
            changes: vec![(clip, Appearance::Footage(smaller))],
            by: Redraw::Style,
            at: 0,
        },
    );
    let project = with_effects(
        &project,
        clip,
        vec![EffectSlot::new(effect(
            Kind::GaussianBlur,
            &[("radius", 40.0)],
        ))],
    );
    let composite = studio.compose(&request(project, 30, 0.25));
    let rgba = pixels(&composite.picture);
    assert!(
        pixel_at(&rgba, 480, 147, 135)[0] > 3,
        "the blur spreads past the footage"
    );
    assert!(is_near(pixel_at(&rgba, 480, 10, 135), BLACK, 0));
}

#[test]
fn unreadable_footage_is_reported_offline() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let missing = cut_project::Media::new("/nowhere/lost.mkv", std::time::Duration::from_secs(4))
        .with_picture(cut_project::Picture {
            width: 640,
            height: 360,
            rate: cut_project::Rate::PAL,
        });
    let (project, clip) = place(&framed_project(), missing, 1);
    let media = project
        .locate(clip)
        .expect("the clip")
        .1
        .media()
        .expect("footage");

    let composite = studio.compose(&request(project, 10, 0.25));
    assert!(composite.offline.contains(&media));
    assert!(is_near(
        pixel_at(&pixels(&composite.picture), 480, 240, 135),
        BLACK,
        0
    ));
}

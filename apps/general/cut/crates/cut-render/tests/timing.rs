use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::support::{Studio, apply, band, effect, encode_test_video, engine, insert, place};
use cut_project::{Art, Edit, EffectSlot, Graphic, Placement, Project};
use cut_render::{Playing, Request};
use rikka_effects::Kind;
use rikka_figure::Lettering;
use rikka_image_edit::Imaging;
use rikka_types::Color;

const FRAMES: u32 = 48;
const BUDGET: Duration = Duration::from_micros(16_700);

fn studio_project(media: cut_project::Media) -> Project {
    let (project, _) = place(&Project::new(), media, 0);
    let title = Graphic::new(Art::text(Lettering {
        text: "Rikka Cut プレビュー".to_owned(),
        size: 96.0,
        weight: Lettering::BOLD,
        color: Color::WHITE,
        ..Lettering::default()
    }))
    .with_placement(Placement {
        y: 360.0,
        ..Placement::default()
    });
    let (project, _) = insert(&project, title, 1);
    let blurred = band(600.0, 400.0).with_placement(Placement {
        x: -480.0,
        y: -200.0,
        ..Placement::default()
    });
    let (project, blur) = insert(&project, blurred, 2);
    let project = apply(
        &project,
        Edit::Effects {
            clip: blur,
            effects: vec![EffectSlot::new(effect(
                Kind::GaussianBlur,
                &[("radius", 20.0)],
            ))],
        },
    );
    let hidden = band(400.0, 300.0).with_placement(Placement {
        x: 480.0,
        y: -200.0,
        ..Placement::default()
    });
    let (project, mosaic) = insert(&project, hidden, 3);
    apply(
        &project,
        Edit::Effects {
            clip: mosaic,
            effects: vec![EffectSlot::new(effect(Kind::Mosaic, &[("cell", 24.0)]))],
        },
    )
}

fn timed(
    studio: &mut Studio,
    imaging: &Imaging,
    project: &Arc<Project>,
    scale: f32,
) -> Vec<Duration> {
    let since = Instant::now();
    let mut durations = Vec::new();
    for frame in 0..i64::from(FRAMES) {
        let request = Request::new(Arc::clone(project), frame)
            .scale(scale)
            .playing(Some(Playing {
                from: frame as f64,
                since,
                speed: 1.0,
            }));
        studio.ready_inputs(&request);
        imaging.compute().wait_idle().expect("idle");
        let started = Instant::now();
        let inputs = studio.compositor.collect_inputs(&request, frame);
        let composite = studio
            .compositor
            .compose(&request, frame, &inputs)
            .expect("composed");
        composite.picture.shown().expect("shown");
        imaging.compute().wait_idle().expect("idle");
        durations.push(started.elapsed());
    }
    durations.sort();
    durations
}

fn report(name: &str, durations: &[Duration]) -> Duration {
    let median = durations[durations.len() / 2];
    println!(
        "{name}: best {:.2} ms, median {:.2} ms, worst {:.2} ms over {} frames",
        durations[0].as_secs_f64() * 1e3,
        median.as_secs_f64() * 1e3,
        durations[durations.len() - 1].as_secs_f64() * 1e3,
        durations.len()
    );
    median
}

#[test]
fn typical_1080p_frame_fits_60_fps_budget() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let directory = tempfile::tempdir().expect("a folder");
    let Some(media) = encode_test_video(
        &directory.path().join("shot.mkv"),
        1920,
        1080,
        30,
        FRAMES + 2,
    ) else {
        return;
    };
    let project = Arc::new(studio_project(media));
    timed(&mut studio, &engine.imaging, &project, 1.0);

    let full = report(
        "full 1920×1080",
        &timed(&mut studio, &engine.imaging, &project, 1.0),
    );
    let half = report(
        "half 960×540",
        &timed(&mut studio, &engine.imaging, &project, 0.5),
    );

    assert!(full < BUDGET, "a full frame takes {full:?}");
    assert!(half < full);
}

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use crate::support::{apply, engine, insert, square};
use cut_project::{
    Appearance, Edit, KeyChange, Placement, PlacementField, Project, Property, Redraw,
};
use cut_render::{Playing, RenderedFrame, Renderer, Request};
use rikka_types::Color;

const FRAME_TIMEOUT: Duration = Duration::from_secs(20);

fn started(engine: &crate::support::Engine) -> (Renderer, Receiver<RenderedFrame>) {
    let (sender, frames) = channel();
    let sender = rikka_sync::Mutex::new(sender);
    let renderer = Renderer::start(engine.imaging.clone(), move |frame: &RenderedFrame| {
        let _ = sender.lock().send(frame.clone());
    })
    .expect("a renderer");
    (renderer, frames)
}

fn sliding() -> Project {
    let start = square(100.0, Color::WHITE).with_placement(Placement {
        x: -800.0,
        ..Placement::default()
    });
    let (project, clip) = insert(&Project::new(), start.clone(), 0);
    let project = apply(
        &project,
        Edit::Key {
            clip,
            property: Property::Placement(PlacementField::Position),
            change: KeyChange::Add { at: 0 },
        },
    );
    apply(
        &project,
        Edit::Redraw {
            changes: vec![(
                clip,
                Appearance::Graphic(start.with_placement(Placement {
                    x: 800.0,
                    ..Placement::default()
                })),
            )],
            by: Redraw::Move,
            at: 59,
        },
    )
}

#[test]
fn paused_request_is_published_once() {
    let engine = engine();
    let (renderer, published) = started(&engine);
    let request = Request::new(Arc::new(sliding()), 12).scale(0.25);

    renderer.submit(request.clone());
    let first = published.recv_timeout(FRAME_TIMEOUT).expect("a frame");
    assert_eq!(first.frame, 12);
    assert!(first.exact);
    assert_eq!((first.picture.width(), first.picture.height()), (480, 270));

    renderer.submit(request);
    assert!(published.recv_timeout(Duration::from_millis(300)).is_err());
    assert!(renderer.is_current());
}

#[test]
fn playback_publishes_clock_frames() {
    let engine = engine();
    let (renderer, published) = started(&engine);
    let since = Instant::now();
    let request = Request::new(Arc::new(sliding()), 0)
        .scale(0.25)
        .playing(Some(Playing {
            from: 0.0,
            since,
            speed: 1.0,
        }));

    renderer.submit(request);
    let mut frames = Vec::new();
    while since.elapsed() < Duration::from_millis(900) {
        if let Ok(frame) = published.recv_timeout(Duration::from_millis(100)) {
            frames.push((frame.frame, Instant::now()));
        }
    }
    drop(renderer);

    assert!(frames.len() >= 10, "{} frames came", frames.len());
    assert!(frames.windows(2).all(|pair| pair[0].0 < pair[1].0));
    let (last, when) = frames[frames.len() - 1];
    let due = when.duration_since(since).as_secs_f64() * 30.0;
    assert!(
        last as f64 >= due - 2.0,
        "frame {last} came when {due:.1} was due"
    );
}

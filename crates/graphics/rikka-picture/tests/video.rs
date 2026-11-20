use std::sync::{Arc, Mutex};
use std::time::Duration;

use rikka_picture::consume::{VideoCommand, VideoConsumer};
use rikka_picture::{VideoController, VideoEvent, VideoState};

fn heard(controller: &VideoController) -> (Arc<Mutex<Vec<VideoEvent>>>, rikka_picture::VideoWatch) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let watch = controller.watch(move |event| sink.lock().unwrap().push(event));
    (events, watch)
}

#[test]
fn watchers_hear_state_changes_and_progress_once_each() {
    let controller = VideoController::open("/clip.webm");
    let (events, _watch) = heard(&controller);

    controller.set_state(VideoState::Paused);
    controller.set_state(VideoState::Paused);
    controller.set_position(Duration::from_millis(40));
    controller.set_position(Duration::from_millis(40));
    controller.set_state(VideoState::Playing);
    controller.fail("broken stream".to_owned());

    assert_eq!(
        *events.lock().unwrap(),
        [
            VideoEvent::Changed,
            VideoEvent::Progressed,
            VideoEvent::Changed,
            VideoEvent::Changed,
        ]
    );
    assert_eq!(controller.state(), VideoState::Failed);
    assert_eq!(controller.error().as_deref(), Some("broken stream"));
}

#[test]
fn a_dropped_watch_hears_nothing() {
    let controller = VideoController::open("/clip.webm");
    let (events, watch) = heard(&controller);
    drop(watch);

    controller.set_state(VideoState::Playing);

    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn commands_reach_the_player_in_order() {
    let controller = VideoController::open("/clip.webm");
    controller.play();
    controller.seek(Duration::from_secs(3));
    controller.pause();

    assert_eq!(
        controller.take_commands(),
        [
            VideoCommand::Play,
            VideoCommand::Seek(Duration::from_secs(3)),
            VideoCommand::Pause,
        ]
    );
    assert!(controller.take_commands().is_empty());
}

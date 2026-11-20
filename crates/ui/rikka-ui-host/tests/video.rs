use rikka_picture::{VideoController, VideoState};
use rikka_ui::{ImageSource, TextureResolver};
use rikka_ui_host::Resources;

#[test]
fn drawn_video_fails_without_a_player() {
    let controller = VideoController::open("clip.mp4");
    let mut resources = Resources::new();
    let source = ImageSource::Video(controller.clone());
    assert_eq!(resources.resolve(&source), None);
    assert_eq!(controller.state(), VideoState::Failed);
}

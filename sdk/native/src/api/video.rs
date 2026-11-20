use std::sync::Arc;

use rikka_widgets as widgets;

use crate::abi::{AnyThread, Closure, violation};
use crate::api::elements::picture;
use crate::ffi;
use crate::media;
use crate::values::{fraction, seconds};
use crate::view::View;
use crate::window::Window;

#[derive(Debug)]
pub(crate) struct Video(Arc<media::Video>);

pub(crate) fn video_new(path: &str) -> Arc<Video> {
    if path.is_empty() {
        violation("the path is empty");
    }
    Arc::new(Video(media::Video::open(path)))
}

pub(crate) fn video_play(video: &Video) {
    video.0.play();
}

pub(crate) fn video_pause(video: &Video) {
    video.0.pause();
}

pub(crate) fn video_stop(video: &Video) {
    video.0.stop();
}

pub(crate) fn video_seek(video: &Video, at: f64) {
    video.0.seek(seconds(at, "position"));
}

pub(crate) fn video_on_end(
    video: &Video,
    window: &Window,
    callback: Closure<ffi::ActionFn, AnyThread>,
) {
    video.0.on_end(window.listener(callback));
}

pub(crate) fn video_state(video: &Video) -> ffi::PlaybackState {
    video.0.state()
}

pub(crate) fn video_is_playing(video: &Video) -> bool {
    video.0.is_playing()
}

pub(crate) fn video_position(video: &Video) -> f64 {
    video.0.position().as_secs_f64()
}

pub(crate) fn video_duration(video: &Video) -> f64 {
    video
        .0
        .duration()
        .map_or(0.0, |duration| duration.as_secs_f64())
}

pub(crate) fn video_volume(video: &Video) -> f64 {
    f64::from(video.0.settings().volume)
}

pub(crate) fn video_set_volume(video: &Video, volume: f64) {
    let volume = fraction(volume, "volume");
    video
        .0
        .set_settings(|settings: &mut media::Settings| settings.volume = volume);
}

pub(crate) fn video_looping(video: &Video) -> bool {
    video.0.settings().looping
}

pub(crate) fn video_set_looping(video: &Video, looping: bool) {
    video
        .0
        .set_settings(|settings: &mut media::Settings| settings.looping = looping);
}

pub(crate) fn video_width(video: &Video) -> f32 {
    video.0.size().map_or(0.0, |(width, _)| width)
}

pub(crate) fn video_height(video: &Video) -> f32 {
    video.0.size().map_or(0.0, |(_, height)| height)
}

pub(crate) fn video_error_message(video: &Video) -> String {
    video.0.error_message()
}

pub(crate) fn video_view(view: &mut View, video: &Video) {
    picture(view, widgets::video(video.0.controller()));
}

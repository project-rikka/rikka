use std::sync::Arc;

use crate::abi::{AnyThread, Closure, violation};
use crate::ffi;
use crate::media;
use crate::values::{fraction, seconds};
use crate::window::Window;

#[derive(Debug)]
pub(crate) struct Sound(Arc<media::Sound>);

pub(crate) fn sound_new(path: &str) -> Arc<Sound> {
    if path.is_empty() {
        violation("the path is empty");
    }
    Arc::new(Sound(media::Sound::open(path)))
}

pub(crate) fn sound_play(sound: &Sound) {
    sound.0.play();
}

pub(crate) fn sound_pause(sound: &Sound) {
    sound.0.pause();
}

pub(crate) fn sound_stop(sound: &Sound) {
    sound.0.stop();
}

pub(crate) fn sound_seek(sound: &Sound, at: f64) {
    sound.0.seek(seconds(at, "position"));
}

pub(crate) fn sound_on_end(
    sound: &Sound,
    window: &Window,
    callback: Closure<ffi::ActionFn, AnyThread>,
) {
    sound.0.on_end(window.listener(callback));
}

pub(crate) fn sound_state(sound: &Sound) -> ffi::PlaybackState {
    sound.0.state()
}

pub(crate) fn sound_is_playing(sound: &Sound) -> bool {
    sound.0.is_playing()
}

pub(crate) fn sound_position(sound: &Sound) -> f64 {
    sound.0.position().as_secs_f64()
}

pub(crate) fn sound_duration(sound: &Sound) -> f64 {
    sound
        .0
        .duration()
        .map_or(0.0, |duration| duration.as_secs_f64())
}

pub(crate) fn sound_volume(sound: &Sound) -> f64 {
    f64::from(sound.0.settings().volume)
}

pub(crate) fn sound_set_volume(sound: &Sound, volume: f64) {
    let volume = fraction(volume, "volume");
    sound
        .0
        .set_settings(|settings: &mut media::Settings| settings.volume = volume);
}

pub(crate) fn sound_looping(sound: &Sound) -> bool {
    sound.0.settings().looping
}

pub(crate) fn sound_set_looping(sound: &Sound, looping: bool) {
    sound
        .0
        .set_settings(|settings: &mut media::Settings| settings.looping = looping);
}

pub(crate) fn sound_error_message(sound: &Sound) -> String {
    sound.0.error_message()
}

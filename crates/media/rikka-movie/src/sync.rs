use std::time::Duration;

use rikka_picture::VideoState;

const ARRIVED_WITHIN: Duration = Duration::from_millis(400);
const DRIFT: Duration = Duration::from_millis(600);

pub(crate) fn has_arrived(target: Duration, picture: Duration, state: VideoState) -> bool {
    state == VideoState::Ended || picture.abs_diff(target) < ARRIVED_WITHIN
}

pub(crate) fn has_drifted(picture: Duration, sound: Duration) -> bool {
    picture.abs_diff(sound) > DRIFT
}

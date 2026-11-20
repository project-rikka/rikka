use rikka_seam::SeamClient;
use rikka_session::replay::Save;
use shell_kit::detached;

use crate::replays::{self, Strings};

pub(super) fn save_now(seam: SeamClient) {
    let words = Strings::now();
    detached("rikka-shell-replay", move || {
        match replays::save(&Save::default()) {
            Ok(saved) => words.notify_saved(&saved, &seam),
            Err(error) => {
                tracing::warn!("failed to save replay: {error}");
                words.notify_failed(&error.to_string(), &seam);
            }
        }
    });
}

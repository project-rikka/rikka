use rikka_tool::Reply;
use serde_json::json;

use crate::animate::Animate;

impl Animate {
    pub(super) fn move_timeline(&mut self, frame: Option<u32>, play: Option<bool>) -> Reply {
        if let Some(frame) = frame {
            if self.transport.is_playing() {
                self.transport.pause();
            }
            self.go_to(frame);
        }
        if let Some(play) = play
            && play != self.transport.is_playing()
        {
            self.play_pause();
        }
        Reply::json(&json!({
            "frame": self.transport.frame(),
            "playing": self.transport.is_playing(),
        }))
    }
}

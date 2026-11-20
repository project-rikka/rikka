use std::ops::ControlFlow;

use discord_client::BroadcastLine;
use discord_core::VOICE_FRAME_SAMPLES;
use rikka_audio::{AudioCapture, Sent, SoundEncoding, SoundSender};
use rikka_opus::OpusApplication;

use crate::error::Result;

const THREAD: &str = "rikka-discord-soundshare";

#[derive(Debug)]
pub struct Soundshare {
    _sender: SoundSender,
}

impl Soundshare {
    pub fn start(capture: AudioCapture, line: BroadcastLine) -> Result<Soundshare> {
        let encoding = SoundEncoding::new(THREAD, OpusApplication::Audio, VOICE_FRAME_SAMPLES);
        let mut speaking = false;
        let sender = SoundSender::start(capture, encoding, move |sent| {
            match sent {
                Sent::Packet { data, .. } => {
                    if !speaking {
                        speaking = true;
                        line.sound_speaking(true);
                    }
                    line.send_sound(data);
                }
                Sent::Quiet => {
                    speaking = false;
                    line.sound_speaking(false);
                }
            }
            ControlFlow::Continue(())
        })?;
        Ok(Soundshare { _sender: sender })
    }
}

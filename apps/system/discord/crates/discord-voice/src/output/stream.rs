use std::time::Instant;

use discord_core::{UserId, VoicePacket};
use rikka_opus::OpusDecoder;
use rikka_playout::{CHANNELS, CHUNK, Packet, Playout, SpeechType};

use crate::error::Result;
use crate::level::rms_db;
use crate::output::speaking::Speaking;

pub(crate) const OUTPUT_CHANNELS: u16 = CHANNELS as u16;
pub(crate) const CHUNK_SAMPLES: usize = CHUNK * CHANNELS;

#[derive(Debug)]
pub(crate) struct Stream {
    user: Option<UserId>,
    playout: Playout<OpusDecoder>,
    speaking: Speaking,
    pcm: Vec<f32>,
}

#[derive(Debug)]
pub(crate) struct Chunk<'a> {
    pub(crate) audio: Option<&'a [f32]>,
    pub(crate) speaking: Option<bool>,
}

impl Stream {
    pub(crate) fn new(origin: Instant) -> Result<Stream> {
        Ok(Stream {
            user: None,
            playout: Playout::new(OpusDecoder::new(OUTPUT_CHANNELS)?, origin),
            speaking: Speaking::default(),
            pcm: vec![0.0; CHUNK_SAMPLES],
        })
    }

    pub(crate) fn user(&self) -> Option<&UserId> {
        self.user.as_ref()
    }

    pub(crate) fn set_user(&mut self, user: UserId) {
        self.user = Some(user);
    }

    pub(crate) fn is_speaking(&self) -> bool {
        self.speaking.is_on()
    }

    pub(crate) fn push(&mut self, packet: VoicePacket) {
        self.playout.push(Packet {
            timestamp: packet.timestamp,
            arrived: packet.arrived,
            payload: packet.opus,
        });
    }

    pub(crate) fn pull(&mut self, now: Instant) -> Chunk<'_> {
        let speech = self.playout.pull(now, &mut self.pcm);
        let level = (speech == SpeechType::Speech).then(|| rms_db(&self.pcm));
        let speaking = self.speaking.update(level, crate::input::STEP);
        let audio = (speech != SpeechType::Silence).then_some(&self.pcm[..]);
        Chunk { audio, speaking }
    }
}

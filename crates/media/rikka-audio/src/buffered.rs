use crate::frame::{Frame, interpolate};
use rikka_audio_decode::Sound;

pub(crate) struct BufferedSource {
    sound: Sound,
    position: f64,
    channels: usize,
    frames: u64,
}

impl BufferedSource {
    pub(crate) fn new(sound: Sound, start_frame: u64) -> Self {
        let frames = sound.frames();
        BufferedSource {
            channels: usize::from(sound.channels().max(1)),
            sound,
            position: start_frame.min(frames) as f64,
            frames,
        }
    }

    pub(crate) fn channels(&self) -> usize {
        self.channels
    }

    pub(crate) fn position_frames(&self) -> u64 {
        self.position as u64
    }

    pub(crate) fn seek(&mut self, frame: u64) -> u64 {
        let frame = frame.min(self.frames);
        self.position = frame as f64;
        frame
    }

    pub(crate) fn read(&mut self, step: f64, looping: bool, out: &mut Frame) -> bool {
        if self.frames == 0 {
            return false;
        }
        let length = self.frames as f64;
        if looping {
            while self.position >= length {
                self.position -= length;
            }
        } else if self.position >= length {
            return false;
        }

        let base = self.position.floor();
        let t = (self.position - base) as f32;
        let base = base as i64;
        let channels = self.channels;
        let samples = self.sound.samples();
        let tap = |index: i64| {
            let start = self.wrap(index, looping) * channels;
            &samples[start..start + channels]
        };
        interpolate(
            tap(base - 1),
            tap(base),
            tap(base + 1),
            tap(base + 2),
            t,
            out,
        );

        self.position += step;
        true
    }

    fn wrap(&self, index: i64, looping: bool) -> usize {
        let end = self.frames as i64;
        let index = if looping && index >= end {
            index - end
        } else {
            index
        };
        index.clamp(0, end - 1) as usize
    }
}

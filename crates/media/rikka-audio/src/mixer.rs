use std::sync::Arc;

use crate::config::MAX_VOICES;
use crate::param::Param;
use crate::ramp::Ramp;
use crate::suspend::Activity;
use crate::voice::{Voice, VoiceStatus};

const MAX_BLOCK_FRAMES: usize = 1024;

pub(crate) type Retired = Box<Voice>;

pub(crate) struct Mixer {
    voices: Vec<Option<Box<Voice>>>,
    commands: rikka_ring::Consumer<Box<Voice>>,
    retired: rikka_ring::Producer<Retired>,
    master_volume: Param,
    master_gain: Ramp,
    channels: usize,
    mix: Vec<f32>,
    sample_rate: u32,
    activity: Arc<Activity>,
}

impl Mixer {
    pub(crate) fn new(
        sample_rate: u32,
        channels: u16,
        commands: rikka_ring::Consumer<Box<Voice>>,
        retired: rikka_ring::Producer<Retired>,
        master_volume: Param,
        activity: Arc<Activity>,
    ) -> Self {
        let block = MAX_BLOCK_FRAMES * channels as usize;
        Mixer {
            voices: (0..MAX_VOICES).map(|_| None).collect(),
            commands,
            retired,
            master_volume,
            master_gain: Ramp::default(),
            channels: usize::from(channels),
            mix: vec![0.0; block],
            sample_rate,
            activity,
        }
    }

    #[cfg(unix)]
    pub(crate) fn max_block_samples(&self) -> usize {
        self.mix.len()
    }

    pub(crate) fn render(&mut self, output: &mut [f32]) {
        self.drain_commands();
        let chunk_len = MAX_BLOCK_FRAMES * self.channels;
        for chunk in output.chunks_mut(chunk_len) {
            self.render_chunk(chunk);
        }
        self.activity.set_silent(self.is_silent());
    }

    fn is_silent(&self) -> bool {
        self.voices.iter().flatten().all(|voice| voice.is_idle())
    }

    fn render_chunk(&mut self, output: &mut [f32]) {
        let len = output.len();
        let occupied = self.voices.iter().any(Option::is_some);
        if !occupied {
            output.fill(0.0);
            return;
        }

        let mix = &mut self.mix[..len];
        mix.fill(0.0);

        for slot in &mut self.voices {
            let Some(voice) = slot.as_mut() else { continue };
            let status = voice.render(mix, self.channels, self.sample_rate);
            if let VoiceStatus::Finished = status
                && let Some(reservation) = self.retired.reserve()
                && let Some(voice) = slot.take()
            {
                reservation.put(voice);
            }
        }

        self.master_gain
            .apply(mix, self.channels, self.master_volume.get().max(0.0));
        for (sample, value) in output.iter_mut().zip(mix.iter()) {
            *sample = value.clamp(-1.0, 1.0);
        }
    }

    fn drain_commands(&mut self) {
        loop {
            let Some(reservation) = self.retired.reserve() else {
                return;
            };
            let Some(voice) = self.commands.pop() else {
                return;
            };
            start_voice(&mut self.voices, voice, reservation);
        }
    }
}

fn start_voice(
    voices: &mut [Option<Box<Voice>>],
    voice: Box<Voice>,
    reservation: rikka_ring::Reservation<'_, Retired>,
) {
    if let Some(slot) = voices.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(voice);
        return;
    }
    let victim = voices
        .iter_mut()
        .filter(|slot| slot.as_ref().is_some_and(|v| v.is_stealable()))
        .min_by_key(|slot| slot.as_ref().map(|v| v.sequence).unwrap_or(u64::MAX));
    let retired = match victim {
        Some(slot) => slot.replace(voice),
        None => Some(voice),
    };
    if let Some(retired) = retired {
        retired.mark_finished();
        reservation.put(retired);
    }
}

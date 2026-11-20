use std::sync::Arc;

use discord_client::Line;
use discord_core::{VOICE_BITRATE, VOICE_FRAME_SAMPLES};
use rikka_apm::Reference;
use rikka_opus::{OPUS_MAX_PACKET, OpusApplication, OpusEncoder, OpusSignal};
use rikka_worker::EventFd;

use crate::error::Result;
use crate::input::{Chain, Processed, STEP_SAMPLES};
use crate::io::{Capture, Devices};
use crate::krisp::Krisp;
use crate::settings::VoiceSettings;
use crate::voice::outbox::Outbox;
use crate::voice::shared::Shared;
use crate::voice::update::VoiceUpdate;

const EXPECTED_LOSS_PERCENT: u8 = 10;
const MONO: u16 = 1;

pub(crate) struct Sending {
    line: Arc<Line>,
    shared: Arc<Shared>,
    outbox: Outbox,
    capture: Capture,
    chain: Chain,
    encoder: OpusEncoder,
    revision: u64,
    frame: Vec<f32>,
    open: bool,
    preroll: Vec<f32>,
    primed: bool,
    packet: Vec<u8>,
    transmitting: bool,
}

impl Sending {
    pub(crate) fn new(
        line: Arc<Line>,
        shared: Arc<Shared>,
        outbox: Outbox,
        devices: Arc<dyn Devices>,
        settings: &VoiceSettings,
        krisp: Krisp,
    ) -> Result<(Sending, Reference)> {
        let mut encoder = OpusEncoder::new(OpusApplication::Voip, MONO)?;
        encoder.set_signal(OpusSignal::Voice)?;
        encoder.set_bitrate(VOICE_BITRATE)?;
        encoder.set_in_band_fec(true)?;
        encoder.set_expected_loss(EXPECTED_LOSS_PERCENT)?;
        let (chain, reference) = Chain::new(settings, krisp);
        devices.hotplug().watch(shared.send_bell().clone());
        let sending = Sending {
            line,
            shared,
            outbox,
            capture: Capture::new(devices, settings.input.clone(), STEP_SAMPLES),
            chain,
            encoder,
            revision: 0,
            frame: Vec::with_capacity(VOICE_FRAME_SAMPLES),
            open: false,
            preroll: Vec::with_capacity(VOICE_FRAME_SAMPLES),
            primed: false,
            packet: vec![0; OPUS_MAX_PACKET],
            transmitting: false,
        };
        Ok((sending, reference))
    }

    pub(crate) fn run(mut self, bell: &EventFd) {
        let mut step = [0.0; STEP_SAMPLES];
        loop {
            bell.drain();
            if self.shared.stopped() {
                break;
            }
            self.refresh();
            if self.capture.poll() {
                self.frame.clear();
                self.primed = false;
                self.quiet();
                self.outbox.send(VoiceUpdate::DeviceLost);
            }
            while self.capture.next_frame(&mut step) {
                self.handle(&mut step);
            }
            if let Err(error) = self.capture.wait(bell) {
                tracing::warn!(%error, "cannot wait for the microphone");
                break;
            }
        }
        self.quiet();
    }

    fn refresh(&mut self) {
        if let Some((revision, settings)) = self.shared.settings_since(self.revision) {
            self.revision = revision;
            self.chain.configure(&settings);
            self.capture.choose(settings.input);
        }
    }

    fn handle(&mut self, step: &mut [f32; STEP_SAMPLES]) {
        let Processed { open, .. } = self.chain.process(step, self.shared.push_to_talk_pressed());
        self.open |= open;
        self.frame.extend_from_slice(step);
        if self.frame.len() < VOICE_FRAME_SAMPLES {
            return;
        }
        let open = std::mem::take(&mut self.open);
        if self.shared.silenced() || !open {
            self.quiet();
        } else {
            if !self.transmitting {
                self.transmitting = true;
                self.line.speaking(true);
                self.outbox.send(VoiceUpdate::SelfSpeaking(true));
                if self.primed {
                    let preroll = std::mem::take(&mut self.preroll);
                    self.send(&preroll);
                    self.preroll = preroll;
                }
            }
            let frame = std::mem::take(&mut self.frame);
            self.send(&frame);
            self.frame = frame;
        }
        std::mem::swap(&mut self.frame, &mut self.preroll);
        self.frame.clear();
        self.primed = true;
    }

    fn quiet(&mut self) {
        if !self.transmitting {
            return;
        }
        self.transmitting = false;
        self.line.speaking(false);
        self.outbox.send(VoiceUpdate::SelfSpeaking(false));
    }

    fn send(&mut self, mono: &[f32]) {
        match self.encoder.encode(mono, &mut self.packet) {
            Ok(length) => self.line.send(&self.packet[..length]),
            Err(error) => tracing::warn!("failed to encode voice frame: {error}"),
        }
    }
}

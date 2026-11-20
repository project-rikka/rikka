use rikka_hifi_core::{DeviceId, Plan, SourceFormat};

use crate::device::Device;
use crate::engine::backend::Backend;
use crate::engine::{Engine, State};
use crate::event::{Event, Failure, OutputState, Refusal};
use crate::exclusive::reserve::Bus;
use crate::exclusive::{Claim, ExclusiveOutput, HardwareVolume, cards, probe};
use crate::output::Output;
use crate::shared::{Setup, SharedOutput};

impl Engine {
    pub(super) fn output_for(
        &mut self,
        format: &SourceFormat,
        claim: Option<Claim>,
    ) -> Option<Plan> {
        let mut claim = claim;
        if let Output::Exclusive(device) = self.choice.clone()
            && self.exclusive_allowed
        {
            match self.open_exclusive(&device, format, claim.take()) {
                Ok(backend) => {
                    let plan = backend.plan();
                    let state = backend.state();
                    self.backend = Some(backend);
                    self.report_output(state);
                    return Some(plan);
                }
                Err(reason) => {
                    tracing::info!("exclusive output on {device} refused: {reason}");
                    self.report_output(OutputState::FellBack { device, reason });
                }
            }
        }
        drop(claim);
        let sink = match &self.choice {
            Output::Sink(sink) => Some(sink.clone()),
            Output::Shared | Output::Exclusive(_) => None,
        };
        let setup = Setup {
            identity: self.identity.clone(),
            sink: sink.clone(),
            plan: Plan::shared(format),
            volume: self.volume,
        };
        match SharedOutput::open(setup, self.next_output()) {
            Ok(output) => {
                let plan = output.plan();
                self.backend = Some(Backend::Shared {
                    output,
                    sink,
                    graph: None,
                });
                if !matches!(self.choice, Output::Exclusive(_)) {
                    self.report_output(OutputState::Shared);
                }
                Some(plan)
            }
            Err(error) => {
                self.emit(Event::Failed {
                    source: None,
                    failure: Failure::Silent(error.to_string()),
                });
                None
            }
        }
    }

    fn open_exclusive(
        &mut self,
        id: &DeviceId,
        format: &SourceFormat,
        claim: Option<Claim>,
    ) -> Result<Backend, Refusal> {
        let device = cards::find(id).ok_or(Refusal::Missing)?;
        let claim = match claim {
            Some(claim) if claim.card == device.card => claim,
            other => {
                drop(other);
                self.claim(&device)?
            }
        };
        let capabilities = probe::capabilities(device.card, device.id.device)
            .ok_or_else(|| Refusal::Open(format!("{id} could not be probed")))?;
        let plan = Plan::exclusive(format, &capabilities).ok_or(Refusal::Unsupported)?;
        let output = ExclusiveOutput::open(device, plan, &self.next_output())?;
        claim.set_volume(self.volume);
        Ok(Backend::Exclusive {
            output: Box::new(output),
            capabilities,
            claim,
        })
    }

    fn claim(&mut self, device: &Device) -> Result<Claim, Refusal> {
        if self.bus.is_none() {
            self.bus = Bus::session();
        }
        let reservation = match self.bus.as_mut() {
            Some(bus) => Some(bus.claim(
                device.card,
                &self.identity.name,
                &device.name,
                &self.notifier,
            )?),
            None => None,
        };
        Ok(Claim {
            card: device.card,
            volume: HardwareVolume::find(device.card),
            reservation,
        })
    }

    pub(super) fn settled_plan(&self, format: &SourceFormat) -> Option<Plan> {
        self.backend.as_ref()?.plan_for(format)
    }

    pub(super) fn relocate(&mut self) {
        let played = self
            .backend
            .as_ref()
            .and_then(|backend| self.timeline.locate(backend.ring().played_frames()));
        self.restore_played();
        self.waiting = false;
        self.pending.clear();
        let claim = self.backend.take().and_then(Backend::into_claim);
        let Some(mut playing) = self.decoding.take() else {
            drop(claim);
            return;
        };
        let at = played
            .filter(|(source, _)| *source == playing.id())
            .map_or_else(Default::default, |(_, at)| at);
        let Some(plan) = self.output_for(&playing.stream.format, claim) else {
            self.idle();
            return;
        };
        let Some(backend) = &self.backend else {
            return;
        };
        let volume = backend.software_volume(self.volume);
        let cursor = self.cursor(backend.ring());
        backend.set_running(self.state == State::Playing);
        playing.set_plan(plan, volume, &self.replay_gain);
        if playing.stream.is_seekable() {
            playing.stream.seek(at);
        }
        self.timeline.restart(playing.id(), cursor, at, plan.rate);
        self.decoding = Some(playing);
        self.report_path();
    }

    pub(super) fn restore_played(&mut self) {
        let Some(previous) = self.previous.take() else {
            return;
        };
        if self.timeline.current_source() != Some(previous.id()) {
            return;
        }
        if let Some(current) = self.decoding.take() {
            if current.stream.is_seekable() {
                current.stream.seek(Default::default());
            }
            self.next = Some(current.stream);
        }
        self.decoding = Some(previous);
    }

    pub(super) fn idle(&mut self) {
        self.previous = None;
        self.decoding = None;
        self.next = None;
        self.waiting = false;
        self.pending.clear();
        self.timeline.clear();
        self.backend = None;
        self.state = State::Idle;
    }

    pub(super) fn report_output(&mut self, state: OutputState) {
        if self.told_output.as_ref() == Some(&state) {
            return;
        }
        self.told_output = Some(state.clone());
        self.emit(Event::Output(state));
    }
}

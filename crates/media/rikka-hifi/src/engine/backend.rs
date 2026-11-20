use rikka_hifi_core::{
    Capabilities, OutputPath, Plan, SharedPath, SourceFormat, Stage, VolumeControl,
};

use crate::event::OutputState;
use crate::exclusive::{Claim, ExclusiveOutput};
use crate::output::Output;
use crate::ring::RingWriter;
use crate::shared::SharedOutput;

pub(crate) enum Backend {
    Shared {
        output: SharedOutput,
        sink: Option<String>,
        graph: Option<SharedPath>,
    },
    Exclusive {
        output: Box<ExclusiveOutput>,
        capabilities: Capabilities,
        claim: Claim,
    },
}

impl Backend {
    pub(crate) fn plan(&self) -> Plan {
        match self {
            Backend::Shared { output, .. } => output.plan(),
            Backend::Exclusive { output, .. } => output.plan(),
        }
    }

    pub(crate) fn ring(&self) -> &RingWriter {
        match self {
            Backend::Shared { output, .. } => output.ring(),
            Backend::Exclusive { output, .. } => output.ring(),
        }
    }

    pub(crate) fn ring_mut(&mut self) -> &mut RingWriter {
        match self {
            Backend::Shared { output, .. } => output.ring_mut(),
            Backend::Exclusive { output, .. } => output.ring_mut(),
        }
    }

    pub(crate) fn plan_for(&self, format: &SourceFormat) -> Option<Plan> {
        match self {
            Backend::Shared { .. } => Some(Plan::shared(format)),
            Backend::Exclusive { capabilities, .. } => Plan::exclusive(format, capabilities),
        }
    }

    pub(crate) fn matches(&self, choice: &Output) -> bool {
        match (self, choice) {
            (Backend::Shared { sink: None, .. }, Output::Shared) => true,
            (
                Backend::Shared {
                    sink: Some(sink), ..
                },
                Output::Sink(target),
            ) => sink == target,
            (Backend::Exclusive { output, .. }, Output::Exclusive(device)) => {
                output.device().id == *device
            }
            _ => false,
        }
    }

    pub(crate) fn set_running(&self, running: bool) {
        match self {
            Backend::Shared { output, .. } => output.set_active(running),
            Backend::Exclusive { output, .. } => output.set_paused(!running),
        }
    }

    pub(crate) fn set_volume(&self, level: f32) {
        match self {
            Backend::Shared { output, .. } => output.set_volume(level),
            Backend::Exclusive { claim, .. } => claim.set_volume(level),
        }
    }

    pub(crate) fn software_volume(&self, level: f32) -> Option<f32> {
        match self {
            Backend::Exclusive { claim, .. } if !claim.has_hardware_volume() => Some(level),
            _ => None,
        }
    }

    pub(crate) fn output_path(&self, volume: f32, stages: &mut Vec<Stage>) -> OutputPath {
        match self {
            Backend::Shared { output, graph, .. } => {
                let plan = output.plan();
                let graph = graph.clone().unwrap_or_else(|| SharedPath {
                    stream_rate: plan.rate,
                    stream_channels: plan.channels,
                    stream_volume: volume,
                    sink: None,
                    sink_volume: None,
                    clock_rate: None,
                    forced_rate: None,
                    allowed_rates: Vec::new(),
                });
                stages.extend(graph.stages());
                OutputPath::Shared(graph)
            }
            Backend::Exclusive { output, claim, .. } => {
                if claim.has_hardware_volume() {
                    stages.push(Stage::Volume {
                        level: volume,
                        by: VolumeControl::Hardware,
                    });
                }
                OutputPath::Exclusive(output.path())
            }
        }
    }

    pub(crate) fn state(&self) -> OutputState {
        match self {
            Backend::Shared { .. } => OutputState::Shared,
            Backend::Exclusive { output, .. } => OutputState::Exclusive(output.device().id.clone()),
        }
    }

    pub(crate) fn into_claim(self) -> Option<Claim> {
        match self {
            Backend::Exclusive { output, claim, .. } => {
                drop(output);
                Some(claim)
            }
            Backend::Shared { .. } => None,
        }
    }
}

use rikka_pipewire::Connection;

use crate::capture::{CaptureConfig, CaptureDevice, CaptureKind, CaptureSource};
use crate::device_watch::DeviceDirection;
use crate::error::{Error, Result, Role};
use crate::frame::check_channels;
use crate::output::{DEFAULT_CHANNELS, DEFAULT_RATE};

use super::super::graph::{Device, Graph};
use super::apps::Linked;

#[must_use]
fn kind_of(device: &Device) -> CaptureKind {
    match device.direction {
        DeviceDirection::Input => CaptureKind::Microphone,
        DeviceDirection::Output | DeviceDirection::Duplex => CaptureKind::SystemAudio,
    }
}

pub(crate) fn devices() -> Result<Vec<CaptureDevice>> {
    let graph = Graph::read(&Connection::open()?)?;
    Ok(graph
        .devices
        .iter()
        .map(|device| CaptureDevice {
            name: device.label.clone(),
            kind: kind_of(device),
            is_default: graph.is_default(device),
        })
        .collect())
}

#[derive(Clone, Debug)]
pub(super) struct InputPlan {
    pub(super) target: Option<String>,
    pub(super) monitor: bool,
    pub(super) linked: Option<Linked>,
    pub(super) rate: u32,
    pub(super) channels: u16,
    pub(super) period: Option<u32>,
    pub(super) node: Option<String>,
    pub(super) passive: bool,
}

pub(super) fn plan(graph: &Graph, config: &CaptureConfig) -> Result<InputPlan> {
    let (target, monitor, linked) = match &config.source {
        CaptureSource::Microphone => {
            graph.inputs().next().ok_or(Error::NoInputDevice)?;
            (None, false, None)
        }
        CaptureSource::SystemAudio => {
            graph.outputs().next().ok_or(Error::NoSystemAudio)?;
            (None, true, None)
        }
        CaptureSource::OtherApps => (None, false, Some(Linked::Others)),
        CaptureSource::Apps(keys) => (None, false, Some(Linked::Only(keys.clone()))),
        CaptureSource::Named(name) => {
            let device = graph
                .find(name)
                .ok_or_else(|| Error::DeviceNotFound(name.clone()))?;
            let monitor = kind_of(device) == CaptureKind::SystemAudio;
            (Some(device.node.clone()), monitor, None)
        }
    };
    let channels = config.channels.unwrap_or(DEFAULT_CHANNELS);
    check_channels(channels, Role::Input)?;
    let rate = config
        .sample_rate
        .or(graph.clock_rate)
        .filter(|rate| *rate > 0)
        .unwrap_or(DEFAULT_RATE);
    Ok(InputPlan {
        target,
        monitor,
        linked,
        rate,
        channels,
        period: config.period,
        node: config.node.clone(),
        passive: config.passive,
    })
}

use std::collections::BTreeMap;

use rikka_compositor_kms::{Connector, Device};
use rikka_compositor_types::OutputId;

use crate::color::HdrDisplay;
use crate::outputs::{Head, Mode, OutputConfig, Outputs};

pub(super) const UNKNOWN: &str = "Unknown";

#[derive(Debug, Default)]
pub(super) struct Rescan {
    pub(super) gone: Vec<OutputId>,
    pub(super) changed: bool,
}

#[derive(Debug, Default)]
pub(super) struct Connectors {
    known: BTreeMap<OutputId, Connector>,
    leasable: Vec<Connector>,
}

impl Connectors {
    pub(super) fn rescan(
        &mut self,
        device: &Device,
        outputs: &mut Outputs,
        named: impl Fn(&Connector) -> String,
    ) -> Rescan {
        let all = match device.connectors() {
            Ok(all) => all,
            Err(error) => {
                tracing::warn!("cannot enumerate connectors: {error}");
                return Rescan::default();
            }
        };
        let (leasable, desktop): (Vec<Connector>, Vec<Connector>) =
            all.into_iter().partition(Connector::non_desktop);
        let leasable_changed = leasable.len() != self.leasable.len()
            || leasable.iter().any(|connector| {
                !self
                    .leasable
                    .iter()
                    .any(|leasable| leasable.id() == connector.id())
            });
        for connector in &leasable {
            tracing::debug!("{} is non-desktop; reserved for leasing", connector.name());
        }
        self.leasable = leasable;
        let gone: Vec<OutputId> = self
            .known
            .iter()
            .filter(|(_, known)| !desktop.iter().any(|connector| connector.id() == known.id()))
            .map(|(output, _)| *output)
            .collect();
        for output in &gone {
            self.known.remove(output);
            outputs.disconnect(*output);
        }

        let mut changed = !gone.is_empty() || leasable_changed;
        for connector in desktop {
            match self
                .known
                .iter_mut()
                .find(|(_, known)| known.id() == connector.id())
            {
                Some((output, known)) => {
                    let current = outputs.get(*output).map(|output| output.head());
                    let name = current.map_or_else(|| named(&connector), |head| head.name.clone());
                    let head = head(&connector, name);
                    if current != Some(&head) {
                        outputs.describe(*output, head);
                        changed = true;
                    }
                    *known = connector;
                }
                None => {
                    let output = outputs.connect(head(&connector, named(&connector)));
                    self.known.insert(output, connector);
                    changed = true;
                }
            }
        }
        Rescan { gone, changed }
    }

    pub(super) fn get(&self, output: OutputId) -> Option<&Connector> {
        self.known.get(&output)
    }

    pub(super) fn leasable(&self) -> &[Connector] {
        &self.leasable
    }

    pub(super) fn offers(&self, output: OutputId, config: &OutputConfig) -> bool {
        let Some(connector) = self.known.get(&output) else {
            return false;
        };
        !config.enabled || mode(connector, config.mode).is_some()
    }

    pub(super) fn forget(&mut self, outputs: &mut Outputs) -> Vec<OutputId> {
        let gone: Vec<OutputId> = self.known.keys().copied().collect();
        for output in &gone {
            outputs.disconnect(*output);
        }
        self.known.clear();
        self.leasable.clear();
        gone
    }
}

pub(super) fn mode(connector: &Connector, target: Mode) -> Option<rikka_compositor_kms::Mode> {
    connector.mode_matching(target.width, target.height, target.refresh)
}

fn head(connector: &Connector, name: String) -> Head {
    let edid = connector.edid();
    let (width_mm, height_mm) = connector.size_mm().unwrap_or((0, 0));
    Head {
        name,
        make: edid.map_or_else(|| UNKNOWN.to_owned(), |edid| edid.make.clone()),
        model: edid.map_or_else(|| UNKNOWN.to_owned(), |edid| edid.model.clone()),
        serial: edid.and_then(|edid| edid.serial.clone()),
        size_mm: (width_mm, height_mm),
        modes: connector
            .modes()
            .iter()
            .map(|mode| Mode {
                width: mode.width(),
                height: mode.height(),
                refresh: mode.refresh_millihertz(),
            })
            .collect(),
        scale: 1.0,
        hdr: edid
            .filter(|_| connector.signals_pq())
            .and_then(HdrDisplay::from_edid),
    }
}

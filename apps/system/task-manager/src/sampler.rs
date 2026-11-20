use rikka_sysmon::gpu::{Adapter, Gpu};
use rikka_sysmon::{Probe, Snapshot, Topology};
use tasks_process::{Group, Processes};

#[derive(Clone, Debug, Default)]
pub(crate) struct Reading {
    pub(crate) machine: Snapshot,
    pub(crate) adapters: Vec<Adapter>,
    pub(crate) groups: Vec<Group>,
    pub(crate) topology: Topology,
}

#[derive(Debug)]
pub(crate) struct Sampler {
    probe: Probe,
    gpu: Gpu,
    processes: Processes,
}

impl Sampler {
    pub(crate) fn new() -> Self {
        Self {
            probe: Probe::new(),
            gpu: Gpu::new(),
            processes: Processes::new(),
        }
    }

    pub(crate) fn read(&mut self) -> Reading {
        let machine = self.probe.sample();
        let gpu = self.gpu.sample();
        Reading {
            groups: self.processes.sample(&gpu.processes),
            adapters: gpu.adapters,
            topology: self.probe.topology().clone(),
            machine,
        }
    }
}

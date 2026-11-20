use crate::cpu::{Cpu, CpuSample};
use crate::disk::{DiskSample, Disks};
use crate::memory::{self, MemorySample};
use crate::network::{NetworkSample, Networks};
use crate::topology::Topology;

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub cpu: CpuSample,
    pub memory: MemorySample,
    pub disks: Vec<DiskSample>,
    pub networks: Vec<NetworkSample>,
}

#[derive(Debug)]
pub struct Probe {
    topology: Topology,
    cpu: Cpu,
    disks: Disks,
    networks: Networks,
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

impl Probe {
    #[must_use]
    pub fn new() -> Self {
        Self {
            topology: Topology::read(),
            cpu: Cpu::new(),
            disks: Disks::new(),
            networks: Networks::new(),
        }
    }

    #[must_use]
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    pub fn sample(&mut self) -> Snapshot {
        Snapshot {
            cpu: self.cpu.sample(),
            memory: memory::sample(),
            disks: self.disks.sample(),
            networks: self.networks.sample(),
        }
    }
}

use std::path::Path;
use std::sync::Arc;

use crate::cpu::CpuSample;
use crate::disk::DiskSample;
use crate::gpu::Adapter;
use crate::load::Load;
use crate::memory::MemorySample;
use crate::network::{NetworkKind, NetworkSample};
use crate::sensors::Sensors;
use crate::storage::Volume;
use crate::topology::Topology;

#[derive(Clone, Debug, Default)]
pub struct Vitals {
    pub topology: Arc<Topology>,
    pub cpu: CpuSample,
    pub cpu_power: Option<f32>,
    pub load: Load,
    pub memory: MemorySample,
    pub gpus: Vec<Adapter>,
    pub disks: Vec<DiskSample>,
    pub volumes: Vec<Volume>,
    pub networks: Vec<NetworkSample>,
    pub sensors: Sensors,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Traffic {
    pub send: f32,
    pub receive: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Io {
    pub read: f32,
    pub write: f32,
}

impl Vitals {
    #[must_use]
    pub fn cpu_temperature(&self) -> Option<f32> {
        self.sensors.cpu()
    }

    #[must_use]
    pub fn main_gpu(&self) -> Option<&Adapter> {
        self.gpus
            .iter()
            .max_by_key(|adapter| adapter.dedicated.map_or(0, |memory| memory.total))
    }

    #[must_use]
    pub fn traffic(&self) -> Traffic {
        self.networks
            .iter()
            .filter(|link| {
                link.up && matches!(link.kind, NetworkKind::Ethernet | NetworkKind::WiFi)
            })
            .fold(Traffic::default(), |sum, link| Traffic {
                send: sum.send + link.send,
                receive: sum.receive + link.receive,
            })
    }

    #[must_use]
    pub fn io(&self) -> Io {
        self.disks.iter().fold(Io::default(), |sum, disk| Io {
            read: sum.read + disk.read,
            write: sum.write + disk.write,
        })
    }

    #[must_use]
    pub fn system_volume(&self) -> Option<&Volume> {
        self.volumes
            .iter()
            .find(|volume| volume.mount == Path::new("/"))
    }
}

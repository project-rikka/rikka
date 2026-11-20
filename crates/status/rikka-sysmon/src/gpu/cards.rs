use std::collections::HashMap;

use crate::gpu::adapter::{Adapter, Memory, Vendor};
use crate::gpu::fdinfo::{Clients, Usage};
use crate::gpu::nvidia::Nvidia;
use crate::gpu::sysfs::Device;
use crate::gpu::{amdgpu, pci, sysfs};

#[derive(Clone, Debug, Default)]
pub struct GpuSample {
    pub adapters: Vec<Adapter>,
    pub processes: HashMap<i32, f32>,
}

#[derive(Debug)]
struct Card {
    device: Device,
    name: String,
    nvml: Option<u32>,
}

#[derive(Debug)]
pub struct Gpu {
    cards: Vec<Card>,
    nvidia: Option<Nvidia>,
    clients: Clients,
    walk: bool,
    measured_by_clients: bool,
}

impl Default for Gpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Gpu {
    #[must_use]
    pub fn new() -> Self {
        let devices = sysfs::devices();
        let nvidia = devices
            .iter()
            .any(|device| Vendor::from_pci(device.vendor_id) == Vendor::Nvidia)
            .then(Nvidia::open)
            .flatten();
        let walk = devices
            .iter()
            .any(|device| Vendor::from_pci(device.vendor_id) != Vendor::Nvidia);
        let cards = devices
            .into_iter()
            .map(|device| {
                let index = nvidia
                    .as_ref()
                    .filter(|_| Vendor::from_pci(device.vendor_id) == Vendor::Nvidia)
                    .and_then(|nvidia| nvidia.index_of(&device.slot));
                Card {
                    name: index
                        .and_then(|index| nvidia.as_ref()?.name(index))
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| pci::name(device.vendor_id, device.device_id)),
                    nvml: index,
                    device,
                }
            })
            .collect::<Vec<Card>>();
        let measured_by_clients = cards
            .iter()
            .any(|card| card.nvml.is_none() && amdgpu::sample(&card.device).is_none());
        let mut probe = Self {
            cards,
            nvidia,
            clients: Clients::default(),
            walk,
            measured_by_clients,
        };
        probe.adapters();
        probe
    }

    pub fn adapters(&mut self) -> Vec<Adapter> {
        let clients = match self.measured_by_clients {
            true => self.clients.sample(),
            false => Usage::default(),
        };
        self.cards
            .iter()
            .map(|card| adapter(self.nvidia.as_mut(), card, &clients))
            .collect()
    }
    pub fn sample(&mut self) -> GpuSample {
        let mut processes = match &mut self.nvidia {
            Some(nvidia) => nvidia.clients(),
            None => HashMap::new(),
        };
        let clients = if self.walk {
            self.clients.sample()
        } else {
            Usage::default()
        };
        for (pid, share) in &clients.processes {
            let usage = processes.entry(*pid).or_default();
            *usage = usage.max(*share);
        }

        let adapters = self
            .cards
            .iter()
            .map(|card| adapter(self.nvidia.as_mut(), card, &clients))
            .collect();
        GpuSample {
            adapters,
            processes,
        }
    }
}

fn adapter(nvidia: Option<&mut Nvidia>, card: &Card, clients: &Usage) -> Adapter {
    let nvidia = nvidia
        .zip(card.nvml)
        .and_then(|(nvidia, index)| nvidia.sample(index));
    let amd = amdgpu::sample(&card.device);
    let regions = clients.memory.get(&card.device.slot);
    let region = |names: &[&str]| {
        let used: u64 = names
            .iter()
            .filter_map(|name| regions?.get(*name).copied())
            .sum();
        (used > 0).then_some(Memory { used, total: 0 })
    };

    Adapter {
        name: card.name.clone(),
        driver: card.device.driver.clone(),
        slot: card.device.slot.clone(),
        usage: nvidia
            .as_ref()
            .map(|reading| reading.usage)
            .or(amd.as_ref().map(|reading| reading.usage))
            .or_else(|| clients.adapters.get(&card.device.slot).copied())
            .unwrap_or_default(),
        encode: nvidia.as_ref().and_then(|reading| reading.encode),
        decode: nvidia.as_ref().and_then(|reading| reading.decode),
        dedicated: nvidia
            .as_ref()
            .and_then(|reading| reading.memory)
            .or_else(|| amd.as_ref().and_then(|reading| reading.vram))
            .or_else(|| region(&["vram"])),
        shared: amd
            .as_ref()
            .and_then(|reading| reading.gtt)
            .or_else(|| region(&["gtt", "system", "cpu", "memory"])),
        temperature: nvidia
            .as_ref()
            .and_then(|reading| reading.temperature)
            .or_else(|| sysfs::temperature(&card.device)),
        power: nvidia
            .as_ref()
            .and_then(|reading| reading.power)
            .or_else(|| sysfs::power(&card.device)),
    }
}

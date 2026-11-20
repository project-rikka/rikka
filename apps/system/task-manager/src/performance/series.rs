use std::collections::HashMap;

use crate::performance::history::History;
use crate::performance::resource::Resource;
use crate::sampler::Reading;

#[derive(Clone, Debug, Default)]
pub(crate) struct Series {
    pub(crate) primary: History,
    pub(crate) secondary: History,
    pub(crate) cores: Vec<History>,
}

#[derive(Debug, Default)]
pub(crate) struct Charts {
    series: HashMap<Resource, Series>,
    order: Vec<Resource>,
}

impl Charts {
    pub(crate) fn record(&mut self, reading: &Reading) {
        let mut order = vec![Resource::Cpu, Resource::Memory];
        self.push(&Resource::Cpu, reading.machine.cpu.usage, 0.0);
        self.cores(&reading.machine.cpu.cores);
        self.push(
            &Resource::Memory,
            reading.machine.memory.used as f32,
            reading.machine.memory.committed as f32,
        );

        for disk in &reading.machine.disks {
            let resource = Resource::Disk(disk.name.clone());
            self.push(&resource, disk.active, disk.read + disk.write);
            order.push(resource);
        }
        for interface in &reading.machine.networks {
            let resource = Resource::Network(interface.name.clone());
            self.push(&resource, interface.receive, interface.send);
            order.push(resource);
        }
        for adapter in &reading.adapters {
            let resource = Resource::Gpu(adapter.slot.clone());
            let dedicated = adapter
                .dedicated
                .map(|memory| memory.used as f32)
                .unwrap_or_default();
            self.push(&resource, adapter.usage, dedicated);
            order.push(resource);
        }

        self.series.retain(|resource, _| order.contains(resource));
        self.order = order;
    }

    pub(crate) fn get(&self, resource: &Resource) -> Option<&Series> {
        self.series.get(resource)
    }

    pub(crate) fn order(&self) -> &[Resource] {
        &self.order
    }

    fn push(&mut self, resource: &Resource, primary: f32, secondary: f32) {
        let series = self.entry(resource);
        series.primary.push(primary);
        series.secondary.push(secondary);
    }

    fn cores(&mut self, usage: &[f32]) {
        let series = self.entry(&Resource::Cpu);
        series.cores.resize(usage.len(), History::default());
        for (history, usage) in series.cores.iter_mut().zip(usage) {
            history.push(*usage);
        }
    }

    fn entry(&mut self, resource: &Resource) -> &mut Series {
        self.series.entry(resource.clone()).or_default()
    }
}

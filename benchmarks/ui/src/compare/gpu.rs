use rikka_nvml::{Nvml, UsedGpuMemory};

pub(crate) struct GpuProbe {
    nvml: Option<Nvml>,
}

impl GpuProbe {
    #[must_use]
    pub(crate) fn new() -> GpuProbe {
        GpuProbe {
            nvml: rikka_nvml::load().ok(),
        }
    }

    #[must_use]
    pub(crate) fn memory(&self, pid: u32) -> Option<u64> {
        let nvml = self.nvml.as_ref()?;
        let devices = nvml.device_count().ok()?;
        let mut used = None;
        for index in 0..devices {
            let Ok(device) = nvml.device_by_index(index) else {
                continue;
            };
            let Ok(processes) = device.running_graphics_processes() else {
                continue;
            };
            for process in processes.into_iter().filter(|process| process.pid == pid) {
                if let UsedGpuMemory::Used(bytes) = process.used_gpu_memory {
                    *used.get_or_insert(0) += bytes;
                }
            }
        }
        used
    }
}

impl std::fmt::Debug for GpuProbe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GpuProbe")
            .field("nvml", &self.nvml.is_some())
            .finish()
    }
}

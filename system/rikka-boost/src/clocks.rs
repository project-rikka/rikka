use rikka_nvml::{Clock, GpuLockedClocksSetting, Nvml, NvmlError};

use crate::error::{Error, Result};

const GRAPHICS_FLOOR_DIVISOR: u32 = 2;

#[derive(Debug, Clone, Copy)]
struct ClockRange {
    min_mhz: u32,
    max_mhz: u32,
}

#[derive(Debug)]
struct Floor {
    index: u32,
    graphics: Option<ClockRange>,
    memory: Option<u32>,
}

pub(crate) struct Clocks {
    nvml: Nvml,
    floors: Vec<Floor>,
}

impl Clocks {
    pub(crate) fn open() -> Result<Self> {
        let nvml = rikka_nvml::load().map_err(Error::Nvml)?;
        let count = nvml.device_count().map_err(Error::Nvml)?;
        let floors: Vec<Floor> = (0..count)
            .filter_map(|index| match floor(&nvml, index) {
                Ok(floor) => Some(floor),
                Err(error) => {
                    eprintln!("GPU {index}: clocks cannot be locked: {error}");
                    None
                }
            })
            .collect();
        if floors.is_empty() {
            return Err(Error::NoGpu);
        }
        Ok(Self { nvml, floors })
    }

    pub(crate) fn raise(&mut self) {
        let nvml = &self.nvml;
        for floor in &mut self.floors {
            let index = floor.index;
            if let Some(memory) = floor.memory {
                lock_clock(&mut floor.memory, index, "memory", || {
                    nvml.device_by_index(index)?
                        .set_mem_locked_clocks(memory, memory)
                });
            }
            if let Some(range) = floor.graphics {
                lock_clock(&mut floor.graphics, index, "graphics", || {
                    nvml.device_by_index(index)?.set_gpu_locked_clocks(
                        GpuLockedClocksSetting::Numeric {
                            min_clock_mhz: range.min_mhz,
                            max_clock_mhz: range.max_mhz,
                        },
                    )
                });
            }
        }
    }

    pub(crate) fn lower(&self) {
        for floor in &self.floors {
            let reset = self
                .nvml
                .device_by_index(floor.index)
                .and_then(|mut device| {
                    if floor.memory.is_some() {
                        device.reset_mem_locked_clocks()?;
                    }
                    if floor.graphics.is_some() {
                        device.reset_gpu_locked_clocks()?;
                    }
                    Ok(())
                });
            if let Err(error) = reset {
                eprintln!(
                    "GPU {}: failed to reset locked clocks: {error}",
                    floor.index
                );
            }
        }
    }
}

fn lock_clock<T>(
    floor: &mut Option<T>,
    index: u32,
    clock: &str,
    lock: impl FnOnce() -> std::result::Result<(), NvmlError>,
) {
    match lock() {
        Ok(()) => {}
        Err(error @ (NvmlError::NotSupported | NvmlError::NoPermission)) => {
            eprintln!("GPU {index}: {clock} clock cannot be locked: {error}");
            *floor = None;
        }
        Err(error) => eprintln!("GPU {index}: failed to lock {clock} clock: {error}"),
    }
}

fn floor(nvml: &Nvml, index: u32) -> std::result::Result<Floor, NvmlError> {
    let device = nvml.device_by_index(index)?;
    let memory = unless_unsupported(device.supported_memory_clocks())?
        .and_then(|clocks| clocks.into_iter().max());
    let graphics =
        unless_unsupported(device.max_clock_info(Clock::Graphics))?.map(|max_mhz| ClockRange {
            min_mhz: max_mhz / GRAPHICS_FLOOR_DIVISOR,
            max_mhz,
        });
    if memory.is_none() && graphics.is_none() {
        return Err(NvmlError::NotSupported);
    }
    Ok(Floor {
        index,
        graphics,
        memory,
    })
}

fn unless_unsupported<T>(
    result: std::result::Result<T, NvmlError>,
) -> std::result::Result<Option<T>, NvmlError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(NvmlError::NotSupported | NvmlError::NoPermission) => Ok(None),
        Err(error) => Err(error),
    }
}

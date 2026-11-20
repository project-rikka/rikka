mod load;

pub use load::load;
pub use nvml_wrapper::enum_wrappers::device::{Clock, Sampling, TemperatureSensor};
pub use nvml_wrapper::enums::device::{GpuLockedClocksSetting, SampleValue, UsedGpuMemory};
pub use nvml_wrapper::error::NvmlError;
pub use nvml_wrapper::struct_wrappers::device::Sample;
pub use nvml_wrapper::{Device, Nvml};

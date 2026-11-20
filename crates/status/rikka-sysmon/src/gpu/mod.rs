mod adapter;
mod amdgpu;
mod cards;
mod fdinfo;
mod nvidia;
mod pci;
mod sysfs;

pub use adapter::{Adapter, Memory};
pub use cards::{Gpu, GpuSample};

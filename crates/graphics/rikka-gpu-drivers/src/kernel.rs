use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::extend_unique;

const DRM: &str = "/sys/class/drm";

const RENDER_NODE: &str = "renderD";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EglVendor {
    Mesa,
    Nvidia,
}

impl EglVendor {
    pub(crate) fn library(self) -> &'static str {
        match self {
            Self::Mesa => "libEGL_mesa",
            Self::Nvidia => "libEGL_nvidia",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Kernel {
    pub(crate) name: &'static str,
    pub(crate) mesa_vulkan: &'static [&'static str],
    pub(crate) vendor_vulkan: &'static [&'static str],
    pub(crate) egl: &'static [EglVendor],
}

const KERNELS: &[Kernel] = &[
    Kernel {
        name: "nvidia",
        mesa_vulkan: &[],
        vendor_vulkan: &["nvidia_icd"],
        egl: &[EglVendor::Nvidia, EglVendor::Mesa],
    },
    Kernel {
        name: "nouveau",
        mesa_vulkan: &["nouveau"],
        vendor_vulkan: &[],
        egl: &[EglVendor::Mesa],
    },
    Kernel {
        name: "amdgpu",
        mesa_vulkan: &["radeon"],
        vendor_vulkan: &["amd_icd"],
        egl: &[EglVendor::Mesa],
    },
    Kernel {
        name: "radeon",
        mesa_vulkan: &["radeon"],
        vendor_vulkan: &[],
        egl: &[EglVendor::Mesa],
    },
    Kernel {
        name: "i915",
        mesa_vulkan: &["intel", "intel_hasvk"],
        vendor_vulkan: &[],
        egl: &[EglVendor::Mesa],
    },
    Kernel {
        name: "xe",
        mesa_vulkan: &["intel"],
        vendor_vulkan: &[],
        egl: &[EglVendor::Mesa],
    },
    Kernel {
        name: "virtio_gpu",
        mesa_vulkan: &["virtio"],
        vendor_vulkan: &[],
        egl: &[EglVendor::Mesa],
    },
];

pub(crate) fn mesa_manifest_prefix(name: &str) -> String {
    format!("{name}_icd")
}

#[derive(Debug, Default)]
pub(crate) struct Machine {
    pub(crate) kernels: Vec<&'static Kernel>,
    pub(crate) unknown: Vec<PathBuf>,
}

pub(crate) fn machine() -> Machine {
    let mut machine = Machine::default();
    let Ok(entries) = std::fs::read_dir(DRM) else {
        return machine;
    };
    let mut nodes: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.starts_with(RENDER_NODE))
        })
        .collect();
    nodes.sort();
    for node in nodes {
        let kernel = driver_of(&node.join("device"))
            .and_then(|name| KERNELS.iter().find(|kernel| kernel.name == name));
        match kernel {
            Some(kernel) => extend_unique(&mut machine.kernels, [kernel]),
            None => machine.unknown.push(node),
        }
    }
    machine
}

fn driver_of(device: &Path) -> Option<String> {
    let driver = std::fs::read_link(device.join("driver")).ok()?;
    Some(driver.file_name()?.to_str()?.to_owned())
}

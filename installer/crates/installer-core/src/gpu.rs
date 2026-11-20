#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuVendor {
    Amd,
    Intel,
    Nvidia,
    Other,
}

impl GpuVendor {
    #[must_use]
    pub fn from_pci_id(pci_vendor: u16) -> Self {
        match pci_vendor {
            0x1002 => Self::Amd,
            0x8086 => Self::Intel,
            0x10de => Self::Nvidia,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gpu {
    pub vendor: GpuVendor,
    pub device: u16,
    pub model: String,
    pub kernel_module: String,
}

impl Gpu {
    const EARLY_KMS: &'static [&'static str] = &["amdgpu", "i915", "radeon", "xe"];

    #[must_use]
    pub fn early_kms(&self) -> Option<&str> {
        Self::EARLY_KMS
            .contains(&self.kernel_module.as_str())
            .then_some(self.kernel_module.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NvidiaDriver {
    Open,
    Legacy,
}

impl NvidiaDriver {
    const MAXWELL: u16 = 0x1340;
    const TURING: u16 = 0x1E00;

    #[must_use]
    pub fn for_device(device: u16) -> Option<Self> {
        match device {
            ..Self::MAXWELL => None,
            Self::MAXWELL..Self::TURING => Some(Self::Legacy),
            _ => Some(Self::Open),
        }
    }

    #[must_use]
    pub fn older(self, other: Self) -> Self {
        match self == Self::Legacy || other == Self::Legacy {
            true => Self::Legacy,
            false => Self::Open,
        }
    }

    #[must_use]
    pub fn open(self) -> bool {
        self == Self::Open
    }

    #[must_use]
    pub fn package(self) -> &'static str {
        match self {
            Self::Open => "latest",
            Self::Legacy => "legacy_580",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "open kernel modules (latest)",
            Self::Legacy => "proprietary kernel modules (580)",
        }
    }
}

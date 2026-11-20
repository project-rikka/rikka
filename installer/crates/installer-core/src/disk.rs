use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk {
    pub name: String,
    pub path: String,
    pub size: ByteSize,
    pub model: String,
    pub kind: DiskKind,
    pub removable: bool,
    pub read_only: bool,
    pub in_use: bool,
}

impl Disk {
    #[must_use]
    pub fn describe(&self) -> String {
        let model = if self.model.is_empty() {
            self.kind.label()
        } else {
            &self.model
        };
        format!("{}  {}  {model}", self.name, self.size)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiskKind {
    Nvme,
    Ssd,
    Rotational,
    Removable,
    Virtual,
    #[default]
    Unknown,
}

impl DiskKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Nvme => "NVMe",
            Self::Ssd => "SSD",
            Self::Rotational => "HDD",
            Self::Removable => "Removable",
            Self::Virtual => "Virtual",
            Self::Unknown => "Disk",
        }
    }

    #[must_use]
    pub fn trims(self) -> bool {
        matches!(self, Self::Nvme | Self::Ssd | Self::Virtual)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(pub u64);

impl ByteSize {
    pub const MIB: u64 = 1024 * 1024;
    pub const GIB: u64 = 1024 * ByteSize::MIB;

    #[must_use]
    pub const fn from_gib(gib: u64) -> Self {
        Self(gib * Self::GIB)
    }

    #[must_use]
    pub const fn from_mib(mib: u64) -> Self {
        Self(mib * Self::MIB)
    }

    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
        let mut value = self.0 as f64;
        let mut unit = 0;
        while value >= 1024.0 && unit + 1 < UNITS.len() {
            value /= 1024.0;
            unit += 1;
        }
        if unit == 0 {
            write!(f, "{}B", self.0)
        } else {
            write!(f, "{value:.1}{}", UNITS[unit])
        }
    }
}

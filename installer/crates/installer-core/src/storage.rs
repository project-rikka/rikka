use crate::disk::ByteSize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filesystem {
    #[default]
    Btrfs,
    Ext4,
}

impl Filesystem {
    pub const ALL: &'static [Filesystem] = &[Filesystem::Btrfs, Filesystem::Ext4];

    pub const SUBVOLUMES: &'static [(&'static str, &'static str)] = &[
        ("root", ""),
        ("home", "/home"),
        ("nix", "/nix"),
        ("snapshots", "/.snapshots"),
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Btrfs => "btrfs",
            Self::Ext4 => "ext4",
        }
    }

    #[must_use]
    pub fn base_options(self) -> &'static [&'static str] {
        match self {
            Self::Btrfs => &["compress=zstd:1", "noatime"],
            Self::Ext4 => &["noatime"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swap {
    Zram { percent: u8 },
    Partition { gib: u16 },
    None,
}

impl Default for Swap {
    fn default() -> Self {
        Self::Zram { percent: 50 }
    }
}

impl Swap {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Zram { percent } => format!("zram ({percent}% of RAM)"),
            Self::Partition { gib } => format!("partition ({gib} GiB)"),
            Self::None => "none".to_owned(),
        }
    }

    #[must_use]
    pub fn on_disk(self) -> Option<ByteSize> {
        match self {
            Self::Partition { gib } => Some(ByteSize::from_gib(u64::from(gib))),
            Self::Zram { .. } | Self::None => None,
        }
    }
}

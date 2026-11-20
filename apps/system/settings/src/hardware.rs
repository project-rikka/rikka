use std::path::{Path, PathBuf};

const PHYS: &str = "/sys/class/ieee80211";
const ADAPTERS: &str = "/sys/class/bluetooth";
const INTERFACES: &str = "/sys/class/net";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Radio {
    Wireless,
    Bluetooth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Hardware {
    pub(crate) wireless: bool,
    pub(crate) bluetooth: bool,
}

impl Hardware {
    pub(crate) fn probe() -> Self {
        let interfaces = interfaces();
        Self {
            wireless: any(PHYS) || interfaces.iter().any(|path| is_wireless(path)),
            bluetooth: any(ADAPTERS),
        }
    }

    pub(crate) fn has(self, radio: Radio) -> bool {
        match radio {
            Radio::Wireless => self.wireless,
            Radio::Bluetooth => self.bluetooth,
        }
    }
}

fn any(directory: &str) -> bool {
    std::fs::read_dir(directory).is_ok_and(|mut entries| entries.next().is_some())
}

fn interfaces() -> Vec<PathBuf> {
    let Ok(interfaces) = std::fs::read_dir(INTERFACES) else {
        return Vec::new();
    };
    interfaces
        .flatten()
        .map(|interface| interface.path())
        .collect()
}

fn is_wireless(interface: &Path) -> bool {
    interface.join("wireless").exists() || interface.join("phy80211").exists()
}

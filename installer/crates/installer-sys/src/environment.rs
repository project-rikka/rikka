use installer_core::{ByteSize, Catalog, FALLBACK_RELEASE, Gpu, stable_release};

use crate::{
    command::Command,
    host::{Host, Mode},
    probe,
};

#[derive(Clone, Debug)]
pub struct Environment {
    pub uefi: bool,
    pub euid: u32,
    pub release: String,
    pub memory: ByteSize,
    pub online: bool,
    pub timezone: String,
    pub missing: Vec<&'static str>,
    pub gpus: Vec<Gpu>,
    pub catalog: Result<Catalog, String>,
    pub mode: Mode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blocker {
    pub message: String,
    pub fatal: bool,
}

const REQUIRED: &[&str] = &[
    "btrfs",
    "cryptsetup",
    "loadkeys",
    "lsblk",
    "mkfs.btrfs",
    "mkfs.ext4",
    "mkfs.fat",
    "mkswap",
    "mount",
    "nix",
    "nixos-generate-config",
    "nixos-install",
    "sgdisk",
    "swapoff",
    "swapon",
    "sync",
    "udevadm",
    "umount",
    "wipefs",
];

const MEMORY_FLOOR: ByteSize = ByteSize::from_gib(2);

impl Environment {
    #[must_use]
    pub fn probe(host: &dyn Host) -> Self {
        Self {
            uefi: host.exists("/sys/firmware/efi"),
            euid: host.euid(),
            release: release(host),
            memory: memory(host),
            online: online(host),
            timezone: timezone(host),
            missing: REQUIRED
                .iter()
                .copied()
                .filter(|tool| !host.which(tool))
                .collect(),
            gpus: probe::graphics(host),
            catalog: probe::catalog(host).map_err(|error| error.to_string()),
            mode: host.mode(),
        }
    }

    #[must_use]
    pub fn blockers(&self) -> Vec<Blocker> {
        let mut blockers = Vec::new();
        let mut stop = |message: String| {
            blockers.push(Blocker {
                message,
                fatal: true,
            });
        };

        if !self.uefi {
            stop(
                "This machine did not boot through UEFI. Switch the firmware to UEFI boot."
                    .to_owned(),
            );
        }
        if self.mode.is_live() && self.euid != 0 {
            stop("Run as root: sudo rikka-installer".to_owned());
        }
        if !self.missing.is_empty() {
            stop(format!("Missing commands: {}", self.missing.join(", ")));
        }

        if self.memory < MEMORY_FLOOR {
            blockers.push(Blocker {
                message: format!("Only {} of memory; nixos-install may run out.", self.memory),
                fatal: false,
            });
        }
        if !self.online {
            blockers.push(Blocker {
                message: "No default route. Connect with nmtui.".to_owned(),
                fatal: false,
            });
        }

        if let Err(reason) = &self.catalog {
            blockers.push(Blocker {
                message: format!("Cannot read the Rikka catalog: {reason}"),
                fatal: false,
            });
        }

        blockers
    }
}

fn release(host: &dyn Host) -> String {
    host.run(&Command::new("nixos-version"))
        .ok()
        .and_then(|output| stable_release(&output.stdout))
        .unwrap_or_else(|| FALLBACK_RELEASE.to_owned())
}

fn timezone(host: &dyn Host) -> String {
    host.run(&Command::new("timedatectl").args(["show", "--property=Timezone", "--value"]))
        .ok()
        .map(|output| output.stdout.trim().to_owned())
        .filter(|zone| zone.contains('/') || zone == "UTC")
        .unwrap_or_else(|| "UTC".to_owned())
}

fn memory(host: &dyn Host) -> ByteSize {
    let Ok(meminfo) = host.read("/proc/meminfo") else {
        return ByteSize(0);
    };
    meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|kib| kib.parse::<u64>().ok())
        .map_or(ByteSize(0), |kib| ByteSize(kib * 1024))
}

fn online(host: &dyn Host) -> bool {
    ["-4", "-6"].iter().any(|family| {
        host.run(&Command::new("ip").args([*family, "route", "show", "default"]))
            .is_ok_and(|output| !output.stdout.trim().is_empty())
    })
}

use installer_core::{ByteSize, Catalog, Disk, DiskKind, Gpu, GpuVendor, PACKAGES_FLAKE};
use serde::{Deserialize, Deserializer, de};

use crate::{
    command::Command,
    error::{Error, Result},
    host::Host,
};

pub fn disks(host: &dyn Host) -> Result<Vec<Disk>> {
    let output = host.capture(&Command::new("lsblk").args([
        "--json",
        "--bytes",
        "--output",
        "NAME,PATH,SIZE,MODEL,TYPE,RM,RO,ROTA,TRAN,MOUNTPOINTS",
    ]))?;

    let listing: Listing =
        serde_json::from_str(&output).map_err(|_| Error::Parse("lsblk".to_owned()))?;

    let mut disks: Vec<Disk> = listing
        .blockdevices
        .iter()
        .filter(|node| node.is_disk())
        .map(Node::to_disk)
        .collect();

    disks.sort_by_key(|disk| (disk.in_use, disk.removable, disk.name.clone()));
    Ok(disks)
}

pub(crate) fn catalog(host: &dyn Host) -> Result<Catalog> {
    let output = host.capture(&Command::nix().args([
        "eval".to_owned(),
        "--json".to_owned(),
        format!("{PACKAGES_FLAKE}#catalog"),
    ]))?;
    Ok(Catalog::parse(&output)?)
}

pub(crate) fn graphics(host: &dyn Host) -> Vec<Gpu> {
    let Ok(output) = host.capture(&Command::new("udevadm").args(["info", "--export-db"])) else {
        return Vec::new();
    };
    output.split("\n\n").filter_map(gpu).collect()
}

fn gpu(record: &str) -> Option<Gpu> {
    let property = |name: &str| {
        record.lines().find_map(|line| {
            line.strip_prefix("E: ")?
                .strip_prefix(name)?
                .strip_prefix('=')
                .map(str::trim)
        })
    };

    if property("SUBSYSTEM") != Some("pci") {
        return None;
    }
    if u32::from_str_radix(property("PCI_CLASS")?, 16).ok()? >> 16 != 0x03 {
        return None;
    }

    let id = property("PCI_ID")?;
    let (vendor, device) = id.split_once(':')?;

    Some(Gpu {
        vendor: GpuVendor::from_pci_id(u16::from_str_radix(vendor, 16).ok()?),
        device: u16::from_str_radix(device, 16).ok()?,
        model: property("ID_MODEL_FROM_DATABASE")
            .filter(|model| !model.is_empty())
            .unwrap_or(id)
            .to_owned(),
        kernel_module: property("DRIVER").unwrap_or_default().to_owned(),
    })
}

pub fn timezones(host: &dyn Host) -> Result<Vec<String>> {
    if let Ok(output) =
        host.capture(&Command::new("timedatectl").args(["list-timezones", "--no-pager"]))
    {
        let zones = collect_zones(output.lines().map(str::to_owned));
        if !zones.is_empty() {
            return Ok(zones);
        }
    }

    for root in ["/etc/zoneinfo", "/usr/share/zoneinfo"] {
        let Ok(output) = host.capture(&Command::new("find").args([root, "-type", "f"])) else {
            continue;
        };
        let zones = collect_zones(output.lines().filter_map(|path| {
            path.strip_prefix(root)
                .map(|name| name.trim_start_matches('/').to_owned())
        }));
        if !zones.is_empty() {
            return Ok(zones);
        }
    }

    Err(Error::Parse("timedatectl".to_owned()))
}

fn collect_zones(candidates: impl Iterator<Item = String>) -> Vec<String> {
    let mut zones: Vec<String> = candidates
        .map(|zone| zone.trim().to_owned())
        .filter(|zone| {
            (zone.contains('/') || zone == "UTC")
                && !zone.starts_with("posix/")
                && !zone.starts_with("right/")
                && zone
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || "/_+-".contains(ch))
        })
        .collect();
    zones.sort();
    zones.dedup();
    zones
}

#[derive(Debug, Deserialize)]
struct Listing {
    blockdevices: Vec<Node>,
}

#[derive(Debug, Deserialize)]
struct Node {
    name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, deserialize_with = "number")]
    size: u64,
    #[serde(default)]
    model: Option<String>,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default, deserialize_with = "flag")]
    rm: bool,
    #[serde(default, deserialize_with = "flag")]
    ro: bool,
    #[serde(default, deserialize_with = "flag")]
    rota: bool,
    #[serde(default)]
    tran: Option<String>,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
    #[serde(default)]
    children: Vec<Node>,
}

impl Node {
    fn is_disk(&self) -> bool {
        self.kind == "disk"
            && self.size > 0
            && !self.name.starts_with("zram")
            && !self.name.starts_with("ram")
            && !self.name.starts_with("loop")
    }

    fn to_disk(&self) -> Disk {
        Disk {
            name: self.name.clone(),
            path: self
                .path
                .clone()
                .unwrap_or_else(|| format!("/dev/{}", self.name)),
            size: ByteSize(self.size),
            model: self.model.clone().unwrap_or_default().trim().to_owned(),
            kind: self.disk_kind(),
            removable: self.rm,
            read_only: self.ro,
            in_use: self.mounted(),
        }
    }

    fn disk_kind(&self) -> DiskKind {
        match self.tran.as_deref() {
            Some("nvme") => DiskKind::Nvme,
            Some("usb") => DiskKind::Removable,
            _ if self.name.starts_with("vd") => DiskKind::Virtual,
            _ if self.rm => DiskKind::Removable,
            _ if self.rota => DiskKind::Rotational,
            Some(_) => DiskKind::Ssd,
            None => DiskKind::Unknown,
        }
    }

    fn mounted(&self) -> bool {
        self.mountpoints.iter().flatten().any(|at| !at.is_empty())
            || self.children.iter().any(Node::mounted)
    }
}

fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    match Loose::deserialize(deserializer)? {
        Loose::Bool(value) => Ok(value),
        Loose::Number(value) => Ok(value != 0),
        Loose::Text(text) => Ok(matches!(text.as_str(), "1" | "true" | "yes")),
        Loose::Null => Ok(false),
    }
}

fn number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    match Loose::deserialize(deserializer)? {
        Loose::Number(value) => Ok(value),
        Loose::Text(text) => text.parse().map_err(de::Error::custom),
        Loose::Bool(_) | Loose::Null => Ok(0),
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Loose {
    Bool(bool),
    Number(u64),
    Text(String),
    Null,
}

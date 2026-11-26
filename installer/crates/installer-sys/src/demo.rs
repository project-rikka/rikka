use std::{thread, time::Duration};

use crate::{
    command::{Command, Output},
    error::{Error, Result},
    host::{Host, Mode},
    journal::Journal,
};

#[derive(Debug, Default)]
pub struct DemoHost {
    paced: bool,
    journal: Journal,
}

impl DemoHost {
    #[must_use]
    pub fn paced() -> Self {
        Self {
            paced: true,
            ..Self::default()
        }
    }

    fn pause(&self, millis: u64) {
        if self.paced {
            thread::sleep(Duration::from_millis(millis));
        }
    }
}

impl Host for DemoHost {
    fn execute(&self, command: &Command, on_line: &mut dyn FnMut(&str)) -> Result<Output> {
        self.journal.command(command);

        let script = script_for(command);
        let mut stdout = String::new();
        for line in script.text.lines() {
            self.pause(script.millis_per_line);
            on_line(line);
            stdout.push_str(line);
            stdout.push('\n');
        }
        if script.text.is_empty() {
            self.pause(script.millis_per_line);
        }

        Ok(Output {
            status: 0,
            stdout,
            stderr: String::new(),
        })
    }

    fn capture(&self, command: &Command) -> Result<String> {
        self.journal.command(command);
        Ok(script_for(command).text.to_owned())
    }

    fn exists(&self, path: &str) -> bool {
        path == "/sys/firmware/efi"
    }

    fn read(&self, path: &str) -> Result<String> {
        match path {
            "/proc/meminfo" => Ok("MemTotal:       32611256 kB\nMemFree:  1042320 kB\n".to_owned()),
            _ => Err(Error::Io {
                path: path.to_owned(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            }),
        }
    }

    fn read_bytes(&self, path: &str) -> Result<Vec<u8>> {
        Err(Error::Io {
            path: path.to_owned(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        })
    }

    fn write(&self, path: &str, contents: &str, mode: u32) -> Result<()> {
        self.journal.write(path, contents, mode);
        Ok(())
    }

    fn mkdir(&self, path: &str, mode: u32) -> Result<()> {
        self.journal.mkdir(path, mode);
        Ok(())
    }

    fn which(&self, _tool: &str) -> bool {
        true
    }

    fn euid(&self) -> u32 {
        0
    }

    fn mode(&self) -> Mode {
        Mode::Demo
    }

    fn journal(&self) -> Vec<String> {
        self.journal.entries()
    }
}

struct Script {
    text: &'static str,
    millis_per_line: u64,
}

fn script_for(command: &Command) -> Script {
    let joined = command.args.join(" ");
    let (text, millis): (&str, u64) = match command.program.as_str() {
        "lsblk" => (LSBLK, 0),
        "udevadm" if joined.starts_with("info") => (UDEVADM, 0),
        "timedatectl" if joined.starts_with("show") => ("Asia/Tokyo", 0),
        "timedatectl" => (TIMEZONES, 0),
        "nixos-version" => ("26.05.20260830.9a1b2c3 (Yarara)", 0),
        "ip" => (
            "default via 192.168.1.1 dev enp0s31f6 proto dhcp metric 100",
            0,
        ),
        "nix" if joined.contains(" eval ") => (CATALOG, 0),
        "nix" if joined.starts_with("--extra-experimental-features") => (FLAKE_LOCK, 1200),
        "nixos-install" => (NIXOS_INSTALL, 200),
        "nixos-generate-config" => ("", 900),
        "sgdisk" => ("The operation has completed successfully.", 250),
        "cryptsetup" if joined.starts_with("luksFormat") => ("", 2500),
        "cryptsetup" => ("", 700),
        "mkfs.btrfs" => (MKFS_BTRFS, 400),
        "mkfs.ext4" => (MKFS_EXT4, 400),
        "mkfs.fat" => ("mkfs.fat 4.2 (2021-01-31)", 150),
        "mkswap" => (
            "Setting up swapspace version 1, size = 8 GiB (8589930496 bytes)",
            200,
        ),
        _ => ("", 120),
    };

    Script {
        text,
        millis_per_line: millis,
    }
}

const CATALOG: &str = r#"{
  "ade": {"name":"ADE","summary":"Develop software with an agent","offer":"offered","version":"0.1.0"},
  "base": {"name":"Rikka desktop","summary":"The compositor, shell, keyring, screen sharing and system services","offer":"required","version":"0.1.0"},
  "browser": {"name":"Browser","summary":"Browse the web","offer":"selected","version":"0.1.0"},
  "files": {"name":"Files","summary":"Browse the files on this computer","offer":"required","version":"0.1.0"},
  "paint": {"name":"Paint","summary":"Draw and edit pictures","offer":"selected","version":"0.1.0"},
  "remote": {"name":"Remote desktop server","summary":"Serves this session to Rikka Remote Viewer over QUIC","offer":"offered","version":"0.1.0"}
}"#;

const FLAKE_LOCK: &str = r#"warning: creating lock file "/mnt/etc/nixos/flake.lock":
* Added input 'nixpkgs':
    'github:NixOS/nixpkgs/9a1b2c3d4e5f60718293a4b5c6d7e8f901a2b3c4' (2026-08-30)"#;

const LSBLK: &str = r#"{
  "blockdevices": [
    {"name":"nvme0n1","path":"/dev/nvme0n1","size":512110190592,"model":"Samsung SSD 980 PRO 512GB","type":"disk","rm":false,"ro":false,"rota":false,"tran":"nvme","mountpoints":[null],
     "children":[
       {"name":"nvme0n1p1","path":"/dev/nvme0n1p1","size":1073741824,"model":null,"type":"part","rm":false,"ro":false,"rota":false,"tran":null,"mountpoints":[null]},
       {"name":"nvme0n1p2","path":"/dev/nvme0n1p2","size":511034359808,"model":null,"type":"part","rm":false,"ro":false,"rota":false,"tran":null,"mountpoints":[null]}
     ]},
    {"name":"sda","path":"/dev/sda","size":1000204886016,"model":"WDC WD10EZEX-08WN4A0","type":"disk","rm":false,"ro":false,"rota":true,"tran":"sata","mountpoints":[null]},
    {"name":"sdb","path":"/dev/sdb","size":31037849600,"model":"SanDisk Ultra Fit","type":"disk","rm":true,"ro":false,"rota":false,"tran":"usb","mountpoints":[null],
     "children":[
       {"name":"sdb1","path":"/dev/sdb1","size":1073741824,"model":null,"type":"part","rm":true,"ro":false,"rota":false,"tran":null,"mountpoints":["/iso"]}
     ]},
    {"name":"zram0","path":"/dev/zram0","size":8589934592,"model":null,"type":"disk","rm":false,"ro":false,"rota":false,"tran":null,"mountpoints":["[SWAP]"]},
    {"name":"loop0","path":"/dev/loop0","size":1849688064,"model":null,"type":"loop","rm":false,"ro":true,"rota":false,"tran":null,"mountpoints":["/nix/.ro-store"]}
  ]
}"#;

const UDEVADM: &str = "\
P: /devices/pci0000:00/0000:00:02.0
E: DEVPATH=/devices/pci0000:00/0000:00:02.0
E: DRIVER=i915
E: PCI_CLASS=30000
E: PCI_ID=8086:5917
E: PCI_SLOT_NAME=0000:00:02.0
E: SUBSYSTEM=pci
E: ID_VENDOR_FROM_DATABASE=Intel Corporation
E: ID_MODEL_FROM_DATABASE=UHD Graphics 620

P: /devices/pci0000:00/0000:00:01.0/0000:01:00.0
E: DEVPATH=/devices/pci0000:00/0000:00:01.0/0000:01:00.0
E: DRIVER=nouveau
E: PCI_CLASS=30200
E: PCI_ID=10DE:2882
E: PCI_SLOT_NAME=0000:01:00.0
E: SUBSYSTEM=pci
E: ID_VENDOR_FROM_DATABASE=NVIDIA Corporation
E: ID_MODEL_FROM_DATABASE=AD107 [GeForce RTX 4060]

P: /devices/pci0000:00/0000:00:1f.3
E: DEVPATH=/devices/pci0000:00/0000:00:1f.3
E: DRIVER=snd_hda_intel
E: PCI_CLASS=40300
E: PCI_ID=8086:9D71
E: SUBSYSTEM=pci
E: ID_VENDOR_FROM_DATABASE=Intel Corporation
E: ID_MODEL_FROM_DATABASE=Sunrise Point-LP HD Audio";

const TIMEZONES: &str = "\
Africa/Cairo
Africa/Johannesburg
Africa/Lagos
Africa/Nairobi
America/Anchorage
America/Argentina/Buenos_Aires
America/Bogota
America/Chicago
America/Denver
America/Halifax
America/Los_Angeles
America/Mexico_City
America/New_York
America/Sao_Paulo
America/Toronto
America/Vancouver
Asia/Bangkok
Asia/Dubai
Asia/Ho_Chi_Minh
Asia/Hong_Kong
Asia/Jakarta
Asia/Jerusalem
Asia/Kathmandu
Asia/Kolkata
Asia/Manila
Asia/Riyadh
Asia/Seoul
Asia/Shanghai
Asia/Singapore
Asia/Taipei
Asia/Tehran
Asia/Tokyo
Australia/Brisbane
Australia/Melbourne
Australia/Perth
Australia/Sydney
Europe/Amsterdam
Europe/Athens
Europe/Berlin
Europe/Brussels
Europe/Bucharest
Europe/Budapest
Europe/Copenhagen
Europe/Dublin
Europe/Helsinki
Europe/Istanbul
Europe/Kyiv
Europe/Lisbon
Europe/London
Europe/Madrid
Europe/Moscow
Europe/Oslo
Europe/Paris
Europe/Prague
Europe/Rome
Europe/Stockholm
Europe/Vienna
Europe/Warsaw
Europe/Zurich
Pacific/Auckland
Pacific/Honolulu
UTC";

const MKFS_BTRFS: &str = "\
btrfs-progs v6.14
Label:              nixos
UUID:               6b3a1f0c-2e7d-4a9b-8c11-5d0e9f3a2b47
Node size:          16384
Sector size:        4096
Filesystem size:    476.14GiB";

const MKFS_EXT4: &str = "\
mke2fs 1.47.2 (1-Jan-2025)
Creating filesystem with 124862976 4k blocks and 31219712 inodes
Writing superblocks and filesystem accounting information: done";

const NIXOS_INSTALL: &str = "\
copying channel...
building the flake closure...
these 214 derivations will be built:
  /nix/store/8xq...-nixos-system-rikka-26.05.drv
these 1043 paths will be fetched (912.44 MiB download, 3204.11 MiB unpacked):
copying path '/nix/store/1v9...-linux-6.16.9' from 'https://cache.nixos.org'...
copying path '/nix/store/3kd...-systemd-258.4' from 'https://cache.nixos.org'...
copying path '/nix/store/7fp...-glibc-2.42' from 'https://cache.nixos.org'...
copying path '/nix/store/9ab...-networkmanager-1.54.0' from 'https://cache.nixos.org'...
copying path '/nix/store/b2c...-openssh-10.2p1' from 'https://cache.nixos.org'...
copying path '/nix/store/d4e...-git-2.52.0' from 'https://cache.nixos.org'...
building '/nix/store/f6g...-etc.drv'...
building '/nix/store/h8i...-initrd-linux-6.16.9.drv'...
building '/nix/store/j0k...-nixos-system-rikka-26.05.drv'...
copying 1043 paths to '/mnt'...
setting up /etc...
installing the boot loader...
Created \"/mnt/boot/EFI\"
Created \"/mnt/boot/EFI/systemd\"
Created \"/mnt/boot/EFI/BOOT\"
Created \"/mnt/boot/loader\"
Created \"/mnt/boot/loader/entries\"
Copied \"/nix/store/3kd...-systemd-258.4/lib/systemd/boot/efi/systemd-bootx64.efi\" to \"/mnt/boot/EFI/systemd/systemd-bootx64.efi\"
Random seed file /mnt/boot/loader/random-seed successfully written (32 bytes).
Created EFI boot entry \"Linux Boot Manager\".
installation finished!";

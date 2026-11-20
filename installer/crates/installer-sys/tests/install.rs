use installer_core::{Answers, DiskKind, Filesystem};
use installer_sys::{DemoHost, Environment, Event, disks};

use crate::support::{FailingHost, answers, encrypted, finished, position, run};

#[test]
fn install_writes_disk_in_order() {
    let host = DemoHost::default();
    let (journal, events) = run(&host, &answers());
    assert!(finished(&events), "{events:?}");

    let sgdisk: Vec<&String> = journal
        .iter()
        .filter(|entry| entry.starts_with("sgdisk"))
        .collect();
    assert_eq!(sgdisk.len(), 3, "{journal:#?}");
    assert_eq!(sgdisk[0], "sgdisk --zap-all /dev/nvme0n1");
    assert!(sgdisk[1].contains("--new=1:0:+1024M"));
    assert!(sgdisk[1].contains("--typecode=1:ef00"));
    assert!(sgdisk[2].contains("--new=2:0:0"));
    assert!(sgdisk[2].contains("--typecode=2:8304"));

    let made = journal
        .iter()
        .rposition(|entry| entry.starts_with("mkfs."))
        .expect("journal should contain mkfs");
    let settle_index = made
        + journal[made..]
            .iter()
            .position(|entry| entry == "udevadm settle --timeout=30")
            .expect("journal should settle after mkfs");
    let subvolume = position(&journal, "btrfs subvolume create /mnt/home");
    let home = position(
        &journal,
        "mount -t btrfs -o compress=zstd:1,noatime,subvol=home /dev/nvme0n1p2 /mnt/home",
    );
    let first_mount = journal
        .iter()
        .position(|entry| entry.starts_with("mount "))
        .expect("journal should contain a mount");
    assert!(
        settle_index < first_mount && subvolume < home,
        "{journal:#?}"
    );
    for entry in journal.iter().filter(|entry| entry.starts_with("mount ")) {
        assert!(entry.starts_with("mount -t "), "{entry}");
    }

    let locked = position(
        &journal,
        "nix --extra-experimental-features 'nix-command flakes' flake lock /mnt/etc/nixos",
    );
    let installed = position(
        &journal,
        "nixos-install --root /mnt --flake /mnt/etc/nixos#workstation --no-root-passwd --no-channel-copy",
    );
    assert!(locked < installed, "{journal:#?}");
    assert!(
        journal
            .iter()
            .any(|entry| entry.starts_with("mkdir /mnt/etc/nixos-secrets (700)"))
    );
    assert!(
        journal
            .iter()
            .any(|entry| entry.starts_with("write /mnt/etc/nixos-secrets/eldodebug (400,"))
    );
}

#[test]
fn encryption_formats_mapping_not_partition() {
    let host = DemoHost::default();
    let (journal, events) = run(&host, &encrypted());
    assert!(finished(&events));

    position(
        &journal,
        "cryptsetup luksFormat --type luks2 --pbkdf argon2id --batch-mode --key-file - /dev/nvme0n1p2",
    );
    position(
        &journal,
        "cryptsetup open --key-file - --persistent --allow-discards /dev/nvme0n1p2 cryptroot",
    );
    position(
        &journal,
        "mkfs.btrfs --force --label nixos /dev/mapper/cryptroot",
    );
    assert!(
        !journal
            .iter()
            .any(|entry| entry.starts_with("mkfs.") && entry.contains("nvme0n1p2")),
        "the container itself is never formatted"
    );
    assert!(!journal.iter().any(|entry| entry.contains("passphrase")));
}

#[test]
fn failure_releases_acquired_resources() {
    let ext4 = Answers {
        filesystem: Filesystem::Ext4,
        ..answers()
    };
    for (program, answers, given_back) in [
        (
            "nixos-install",
            encrypted(),
            &["umount --recursive /mnt", "cryptsetup close cryptroot"][..],
        ),
        ("sgdisk", answers(), &[][..]),
        ("mount", ext4, &[][..]),
    ] {
        let host = FailingHost::new(program);
        let (journal, events) = run(&host, &answers);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, Event::Failed(message) if message.contains("rigged"))),
            "{program}: {events:?}"
        );
        let failed = journal
            .iter()
            .position(|entry| entry.starts_with(program))
            .expect("rigged program should have run");
        let unwound: Vec<&str> = journal[failed..]
            .iter()
            .filter(|entry| {
                ["umount", "swapoff", "cryptsetup close"]
                    .iter()
                    .any(|undo| entry.starts_with(undo))
            })
            .map(String::as_str)
            .collect();
        assert_eq!(unwound, given_back, "{program}: {journal:#?}");
    }
}

#[test]
fn parses_lsblk_disks() {
    let found = disks(&DemoHost::default()).expect("demo host should answer lsblk");
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].name, "nvme0n1");
    assert_eq!(found[0].kind, DiskKind::Nvme);
    assert!(!found[0].in_use);
    assert!(found.last().expect("three disks").in_use);
}

#[test]
fn reads_rikka_catalog() {
    let environment = Environment::probe(&DemoHost::default());
    let catalog = environment
        .catalog
        .as_ref()
        .expect("demo host should answer nix eval");
    assert!(catalog.find("base").is_some());
    assert!(
        !environment
            .blockers()
            .iter()
            .any(|blocker| blocker.message.contains("catalog"))
    );
}

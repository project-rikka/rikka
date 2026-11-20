use installer_core::{Answers, ByteSize, Error, PartitionPlan, PartitionRole, Swap};

use crate::support::disk;

fn plan(answers: Answers) -> PartitionPlan {
    PartitionPlan::build(&answers).expect("disk should fit the layout")
}

#[test]
fn partitions_follow_kernel_naming() {
    for (path, swap, root) in [
        ("/dev/nvme0n1", Swap::Zram { percent: 50 }, "/dev/nvme0n1p2"),
        ("/dev/mmcblk0", Swap::Partition { gib: 8 }, "/dev/mmcblk0p3"),
        ("/dev/sda", Swap::Partition { gib: 8 }, "/dev/sda3"),
        ("/dev/sda", Swap::Zram { percent: 50 }, "/dev/sda2"),
    ] {
        let plan = plan(Answers {
            disk: Some(disk(ByteSize::from_gib(512), path)),
            swap,
            ..Answers::default()
        });
        assert_eq!(plan.root().node, root, "{path} {swap:?}");
        assert_eq!(
            plan.find(PartitionRole::Swap).is_some(),
            matches!(swap, Swap::Partition { .. })
        );
    }
}

#[test]
fn encryption_maps_root_device() {
    let plan = plan(Answers {
        disk: Some(disk(ByteSize::from_gib(256), "/dev/nvme0n1")),
        encrypt: true,
        ..Answers::default()
    });
    assert_eq!(plan.root().type_code, "8309");
    assert_eq!(plan.root_device(), "/dev/mapper/cryptroot");
}

#[test]
fn rejects_too_small_disk() {
    let answers = Answers {
        disk: Some(disk(ByteSize::from_gib(8), "/dev/sdb")),
        ..Answers::default()
    };
    assert!(matches!(
        PartitionPlan::build(&answers),
        Err(Error::DiskTooSmall { .. })
    ));
}

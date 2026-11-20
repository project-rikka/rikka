use crate::{
    answers::Answers,
    disk::{ByteSize, Disk},
    error::{Error, Result},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartitionRole {
    Esp,
    Swap,
    Root,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionSpec {
    pub role: PartitionRole,
    pub number: u8,
    pub label: &'static str,
    pub type_code: &'static str,
    pub size: Option<ByteSize>,
    pub node: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionPlan {
    pub disk: Disk,
    pub partitions: Vec<PartitionSpec>,
    pub encrypt: bool,
    pub luks_name: &'static str,
}

const ESP_SIZE: ByteSize = ByteSize::from_mib(1024);

const ROOT_MINIMUM: ByteSize = ByteSize::from_gib(12);

const ROOT_TYPE: &str = "8304";

impl PartitionPlan {
    pub fn build(answers: &Answers) -> Result<Self> {
        let disk = answers.disk.clone().ok_or(Error::NoDisk)?;
        let swap = answers.swap.on_disk();

        let reserved = ESP_SIZE.bytes() + swap.map_or(0, ByteSize::bytes) + ROOT_MINIMUM.bytes();
        if disk.size.bytes() < reserved {
            return Err(Error::DiskTooSmall {
                disk: disk.name.clone(),
                size: disk.size.to_string(),
                needed: ByteSize(reserved).to_string(),
            });
        }

        let mut partitions = vec![PartitionSpec {
            role: PartitionRole::Esp,
            number: 1,
            label: "RIKKA-ESP",
            type_code: "ef00",
            size: Some(ESP_SIZE),
            node: partition_node(&disk.path, 1),
        }];

        if let Some(size) = swap {
            partitions.push(PartitionSpec {
                role: PartitionRole::Swap,
                number: 2,
                label: "RIKKA-SWAP",
                type_code: "8200",
                size: Some(size),
                node: partition_node(&disk.path, 2),
            });
        }

        let number = partitions.len() as u8 + 1;
        partitions.push(PartitionSpec {
            role: PartitionRole::Root,
            number,
            label: "RIKKA-ROOT",
            type_code: if answers.encrypt { "8309" } else { ROOT_TYPE },
            size: None,
            node: partition_node(&disk.path, number),
        });

        Ok(Self {
            disk,
            partitions,
            encrypt: answers.encrypt,
            luks_name: Answers::LUKS_NAME,
        })
    }

    #[must_use]
    pub fn find(&self, role: PartitionRole) -> Option<&PartitionSpec> {
        self.partitions.iter().find(|spec| spec.role == role)
    }

    #[must_use]
    pub fn esp(&self) -> &PartitionSpec {
        self.find(PartitionRole::Esp)
            .expect("plan should have an ESP")
    }

    #[must_use]
    pub fn root(&self) -> &PartitionSpec {
        self.find(PartitionRole::Root)
            .expect("plan should have a root partition")
    }

    #[must_use]
    pub fn root_device(&self) -> String {
        if self.encrypt {
            format!("/dev/mapper/{}", self.luks_name)
        } else {
            self.root().node.clone()
        }
    }

    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        self.partitions
            .iter()
            .map(|spec| {
                let size = spec
                    .size
                    .map_or_else(|| "rest".to_owned(), |size| size.to_string());
                let purpose = match spec.role {
                    PartitionRole::Esp => "EFI system (FAT32)",
                    PartitionRole::Swap => "swap",
                    PartitionRole::Root => {
                        if self.encrypt {
                            "LUKS2 container -> root"
                        } else {
                            "root"
                        }
                    }
                };
                format!("{}  {size:>10}  {purpose}", spec.node)
            })
            .collect()
    }
}

fn partition_node(disk_path: &str, number: u8) -> String {
    if disk_path.ends_with(|ch: char| ch.is_ascii_digit()) {
        format!("{disk_path}p{number}")
    } else {
        format!("{disk_path}{number}")
    }
}

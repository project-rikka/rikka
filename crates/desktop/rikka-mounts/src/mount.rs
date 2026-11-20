use std::path::PathBuf;

const KERNEL_FILESYSTEMS: &[&str] = &[
    "autofs",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "debugfs",
    "devpts",
    "devtmpfs",
    "efivarfs",
    "fusectl",
    "hugetlbfs",
    "mqueue",
    "proc",
    "pstore",
    "ramfs",
    "securityfs",
    "selinuxfs",
    "sysfs",
    "tracefs",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    pub at: PathBuf,
    pub source: String,
    pub filesystem: String,
    pub read_only: bool,
    pub no_setuid: bool,
}

impl Mount {
    pub(crate) fn parse(line: &str) -> Option<Mount> {
        let (mount, rest) = line.split_once(" - ")?;
        let mut before = mount.split(' ');
        let at = before.nth(4)?;
        let options = before.next()?;
        let mut after = rest.split(' ');
        let filesystem = after.next()?;
        let source = after.next()?;
        let super_options = after.next().unwrap_or_default();
        let listed = |wanted: &str| {
            [options, super_options]
                .iter()
                .any(|listed| listed.split(',').any(|option| option == wanted))
        };
        Some(Mount {
            at: PathBuf::from(unescaped(at)),
            source: unescaped(source),
            filesystem: filesystem.to_owned(),
            read_only: listed("ro"),
            no_setuid: listed("nosuid"),
        })
    }

    #[must_use]
    pub fn is_storage(&self) -> bool {
        !KERNEL_FILESYSTEMS.contains(&self.filesystem.as_str())
    }
}

fn unescaped(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let octal = bytes
            .get(at + 1..at + 4)
            .filter(|digits| bytes[at] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)))
            .and_then(|digits| {
                u8::try_from(
                    digits
                        .iter()
                        .fold(0u16, |value, d| value * 8 + u16::from(d - b'0')),
                )
                .ok()
            });
        match octal {
            Some(byte) => {
                out.push(byte);
                at += 4;
            }
            None => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

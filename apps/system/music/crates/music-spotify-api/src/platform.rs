use std::path::Path;

const SYSTEM_NAME: &str = "Linux";
const KERNEL_RELEASE: &str = "/proc/sys/kernel/osrelease";
const OS_RELEASE: &str = "/etc/os-release";
const VERSION_ID: &str = "VERSION_ID=";
const UNKNOWN: &str = "0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Platform {
    pub system_name: String,
    pub system_release: String,
    pub system_version: String,
    pub hardware: String,
}

impl Platform {
    #[must_use]
    pub fn current() -> Platform {
        Platform {
            system_name: SYSTEM_NAME.to_owned(),
            system_release: read(Path::new(KERNEL_RELEASE))
                .map(|release| release.trim().to_owned())
                .unwrap_or_else(|| UNKNOWN.to_owned()),
            system_version: read(Path::new(OS_RELEASE))
                .and_then(|release| version_id(&release))
                .unwrap_or_else(|| UNKNOWN.to_owned()),
            hardware: std::env::consts::ARCH.to_owned(),
        }
    }
}

fn read(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) => {
            tracing::debug!("failed to read {}: {error}", path.display());
            None
        }
    }
}

fn version_id(release: &str) -> Option<String> {
    release
        .lines()
        .find_map(|line| line.strip_prefix(VERSION_ID))
        .map(|version| version.trim_matches('"').to_owned())
}

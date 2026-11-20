use std::collections::BTreeSet;

use crate::{
    FALLBACK_RELEASE,
    catalog::{Catalog, Offer},
    disk::Disk,
    gpu::{Gpu, GpuVendor, NvidiaDriver},
    kernel::Kernel,
    keyboard::{KEYBOARDS, Keyboard},
    locale::{LOCALES, Locale},
    secret::Secret,
    storage::{Filesystem, Swap},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Channel {
    #[default]
    Unstable,
    Stable,
}

pub(crate) const RIKKA_NIXPKGS: &str = "rikka/nixpkgs";

impl Channel {
    pub const ALL: &'static [Channel] = &[Channel::Unstable, Channel::Stable];

    #[must_use]
    pub fn label(self, release: &str) -> String {
        match self {
            Self::Unstable => "unstable (the nixpkgs Rikka is built with)".to_owned(),
            Self::Stable => format!("stable (nixos-{release})"),
        }
    }

    #[must_use]
    pub fn source(self, release: &str) -> String {
        match self {
            Self::Unstable => format!("follows {RIKKA_NIXPKGS}"),
            Self::Stable => format!("github:NixOS/nixpkgs/nixos-{release}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Answers {
    pub locale: &'static Locale,
    pub english_messages: bool,
    pub keyboard: &'static Keyboard,
    pub timezone: String,

    pub disk: Option<Disk>,
    pub filesystem: Filesystem,
    pub swap: Swap,
    pub encrypt: bool,
    pub passphrase: Secret,
    pub passphrase_confirm: Secret,

    pub hostname: String,
    pub username: String,
    pub full_name: String,
    pub password: Secret,
    pub password_confirm: Secret,
    pub set_root_password: bool,
    pub root_password: Secret,
    pub root_password_confirm: Secret,

    pub channel: Channel,
    pub kernel: Kernel,
    pub allow_unfree: bool,
    pub openssh: bool,
    pub ssh_password_auth: bool,
    pub nvidia: bool,
    pub sudo_rs: bool,

    pub catalog: Catalog,
    pub apps: BTreeSet<String>,

    pub release: String,
    pub gpus: Vec<Gpu>,
}

impl Default for Answers {
    fn default() -> Self {
        Self {
            locale: &LOCALES[0],
            english_messages: false,
            keyboard: &KEYBOARDS[0],
            timezone: "UTC".to_owned(),

            disk: None,
            filesystem: Filesystem::default(),
            swap: Swap::default(),
            encrypt: false,
            passphrase: Secret::new(),
            passphrase_confirm: Secret::new(),

            hostname: "nixos".to_owned(),
            username: String::new(),
            full_name: String::new(),
            password: Secret::new(),
            password_confirm: Secret::new(),
            set_root_password: false,
            root_password: Secret::new(),
            root_password_confirm: Secret::new(),

            channel: Channel::default(),
            kernel: Kernel::default(),
            allow_unfree: false,
            openssh: true,
            ssh_password_auth: false,
            nvidia: false,
            sudo_rs: false,

            catalog: Catalog::default(),
            apps: BTreeSet::new(),

            release: FALLBACK_RELEASE.to_owned(),
            gpus: Vec::new(),
        }
    }
}

impl Answers {
    pub const LUKS_NAME: &'static str = "cryptroot";

    #[must_use]
    pub fn password_file(&self) -> String {
        format!("{}/{}", crate::SECRETS_DIR, self.username)
    }

    #[must_use]
    pub fn root_password_file() -> String {
        format!("{}/root", crate::SECRETS_DIR)
    }

    pub fn offer(&mut self, catalog: Catalog) {
        self.apps = catalog
            .optional()
            .filter(|component| component.offer == Offer::Selected)
            .map(|component| component.id.clone())
            .collect();
        self.catalog = catalog;
    }

    pub fn choose_app(&mut self, id: &str, chosen: bool) {
        let optional = self
            .catalog
            .find(id)
            .is_some_and(|component| component.offer != Offer::Required);
        if !optional {
            return;
        }
        match chosen {
            true => self.apps.insert(id.to_owned()),
            false => self.apps.remove(id),
        };
    }

    #[must_use]
    pub fn has_gpu(&self, vendor: GpuVendor) -> bool {
        self.gpus.iter().any(|gpu| gpu.vendor == vendor)
    }

    #[must_use]
    pub fn nvidia_offer(&self) -> Option<NvidiaDriver> {
        let drivers: Option<Vec<NvidiaDriver>> = self
            .gpus
            .iter()
            .filter(|gpu| gpu.vendor == GpuVendor::Nvidia)
            .map(|gpu| NvidiaDriver::for_device(gpu.device))
            .collect();
        drivers?.into_iter().reduce(NvidiaDriver::older)
    }

    #[must_use]
    pub fn nvidia_driver(&self) -> Option<NvidiaDriver> {
        self.nvidia.then(|| self.nvidia_offer()).flatten()
    }

    #[must_use]
    pub fn early_kms(&self) -> Vec<&str> {
        let mut modules: Vec<&str> = self.gpus.iter().filter_map(Gpu::early_kms).collect();
        modules.sort_unstable();
        modules.dedup();
        modules
    }

    #[must_use]
    pub fn graphics(&self) -> Vec<String> {
        let nvidia = self.nvidia_driver();
        self.gpus
            .iter()
            .map(|gpu| {
                let with = match gpu.vendor {
                    GpuVendor::Nvidia => nvidia.map_or("nouveau", NvidiaDriver::label),
                    GpuVendor::Intel => "Mesa, intel-media-driver",
                    GpuVendor::Amd | GpuVendor::Other => "Mesa",
                };
                format!("{}: {with}", gpu.model)
            })
            .collect()
    }

    #[must_use]
    pub fn swap_choices(&self) -> Vec<Swap> {
        let mut choices = vec![Swap::Zram { percent: 50 }, Swap::None];
        if !self.encrypt {
            choices.insert(1, Swap::Partition { gib: 8 });
        }
        choices
    }
}

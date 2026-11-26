use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use rikka_age::Encryptor;
use rikka_file_format::JsonFormat;
use rikka_nix_edit::Document;
use rikka_nixos_layout::Module;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::configured::Configured;
use crate::credential::Credential;
use crate::error::{Error, Result};
use crate::file::File;
use crate::origin::Origin;
use crate::preference::Preference;

const FORMAT: JsonFormat = JsonFormat::new("rikka-backup");

pub const PORTABLE: [Module; 7] = [
    Module::Apps,
    Module::Ime,
    Module::Integration,
    Module::Nix,
    Module::Packages,
    Module::Settings,
    Module::System,
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub origin: Origin,
    pub modules: Vec<Configured>,
    pub preferences: Vec<Preference>,
    pub files: Vec<File>,
    pub credentials: Vec<Credential>,
}

impl Bundle {
    pub fn seal(&self, passphrase: &str) -> Result<Vec<u8>> {
        self.seal_with(Encryptor::with_passphrase(passphrase))
    }

    pub fn seal_with(&self, encryptor: Encryptor) -> Result<Vec<u8>> {
        self.validate()?;
        let json = Zeroizing::new(FORMAT.encode(self)?);
        let mut writer = encryptor.wrap(Vec::new())?;
        std::io::Write::write_all(&mut writer, json.as_bytes()).map_err(rikka_age::Error::from)?;
        Ok(writer.finish().map_err(rikka_age::Error::from)?)
    }

    pub fn open(sealed: impl Read, passphrase: &str) -> Result<Bundle> {
        let json = rikka_age::decrypt(passphrase, sealed)?;
        let bundle: Bundle = FORMAT.decode(json.as_slice())?;
        bundle.validate()?;
        Ok(bundle)
    }

    #[must_use]
    pub fn module(&self, module: Module) -> Option<&str> {
        self.modules
            .iter()
            .find(|configured| configured.module == module)
            .map(|configured| configured.text.as_str())
    }

    pub fn documents_for(&self, user: &str) -> Result<BTreeMap<Module, Document>> {
        Configured::documents(&self.modules, Some(&self.origin.user), user)
    }

    fn validate(&self) -> Result<()> {
        let mut modules = BTreeSet::new();
        for configured in &self.modules {
            if !PORTABLE.contains(&configured.module) {
                return Err(Error::Invalid(format!(
                    "the {} module belongs to one computer",
                    configured.module.name()
                )));
            }
            if !modules.insert(configured.module) {
                return Err(Error::Invalid(format!(
                    "the {} module appears twice",
                    configured.module.name()
                )));
            }
        }
        let mut files = BTreeSet::new();
        for file in &self.files {
            file.validate()?;
            if !files.insert((file.base(), file.path().to_owned())) {
                return Err(Error::Invalid(format!(
                    "`{}` appears twice",
                    file.path().display()
                )));
            }
        }
        let mut preferences = BTreeSet::new();
        for preference in &self.preferences {
            if !preferences.insert((&preference.domain, &preference.name)) {
                return Err(Error::Invalid(format!(
                    "{}/{} appears twice",
                    preference.domain, preference.name
                )));
            }
        }
        Ok(())
    }
}

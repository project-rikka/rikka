use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rikka_backup::{Bundle, Configured, Origin, PORTABLE};
use rikka_nix::{Document, Module, Modules};
use rikka_seam::SeamClient;
use rikka_settings::Value;
use settings_backup::{Dirs, Identity, Published, Repository};
use settings_declared::SettingId;
use zeroize::Zeroizing;

use crate::configuration::{Values, Write, survey};

pub(crate) type Documents = BTreeMap<Module, Document>;

const HOST_NAME: &str = "/proc/sys/kernel/hostname";
const CLONE: &str = "configuration-repository";
const CHECKOUT: &str = "checkout";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Account {
    pub(crate) name: String,
    pub(crate) full_name: String,
    pub(crate) dirs: Dirs,
}

#[derive(Debug)]
pub(crate) struct Exported {
    pub(crate) path: PathBuf,
    pub(crate) moved: usize,
}

#[derive(Debug)]
pub(crate) struct Restored {
    pub(crate) documents: Documents,
    pub(crate) skipped: Vec<String>,
}

impl Account {
    pub(crate) fn current() -> Result<Account, String> {
        let entry = rikka_dirs::passwd::current()
            .ok_or_else(|| "the current account has no passwd entry".to_owned())?;
        let paths = rikka_dirs::Paths::resolve().map_err(|error| error.to_string())?;
        Ok(Account {
            full_name: entry.display_name().to_owned(),
            dirs: Dirs::from_paths(&entry.home, &paths),
            name: entry.name,
        })
    }

    fn origin(&self) -> Result<Origin, String> {
        let home = self
            .dirs
            .home
            .to_str()
            .ok_or_else(|| format!("{} is not UTF-8", self.dirs.home.display()))?;
        Ok(Origin {
            host: host_name()?,
            user: self.name.clone(),
            home: home.to_owned(),
            created: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs()),
        })
    }

    fn identity(&self) -> Result<Identity, String> {
        Ok(Identity {
            name: self.full_name.clone(),
            email: format!("{}@{}", self.name, host_name()?),
        })
    }
}

pub(crate) fn host_name() -> Result<String, String> {
    std::fs::read_to_string(HOST_NAME)
        .map(|name| name.trim().to_owned())
        .map_err(|error| format!("failed to read the computer name: {error}"))
}

fn configured() -> Result<Vec<Configured>, String> {
    let modules = Modules::system().map_err(|error| error.to_string())?;
    Ok(PORTABLE
        .into_iter()
        .map(|module| Configured {
            module,
            text: modules.get(module).source().to_owned(),
        })
        .collect())
}

pub(crate) fn export(
    account: &Account,
    target: &Path,
    passphrase: &str,
    sign_ins: bool,
) -> Result<Exported, String> {
    let failed = |error: settings_backup::Error| error.to_string();
    let (files, credentials) = match sign_ins {
        true => {
            let secrets =
                rikka_secrets::blocking::Secrets::shared().map_err(|error| error.to_string())?;
            (
                settings_backup::collect_files(&account.dirs).map_err(failed)?,
                settings_backup::collect_credentials(&secrets).map_err(failed)?,
            )
        }
        false => (Vec::new(), Vec::new()),
    };
    let bundle = Bundle {
        origin: account.origin()?,
        modules: configured()?,
        preferences: settings_backup::preferences(&survey()?),
        files,
        credentials,
    };
    settings_backup::export(&bundle, passphrase, target).map_err(failed)?;
    Ok(Exported {
        path: target.to_owned(),
        moved: bundle.credentials.len(),
    })
}

pub(crate) fn open(path: &Path, passphrase: &Zeroizing<String>) -> Result<Bundle, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    Bundle::open(file, passphrase).map_err(|error| error.to_string())
}

pub(crate) fn restore(
    account: &Account,
    seam: Option<&SeamClient>,
    bundle: &Bundle,
    source: &Path,
) -> Result<Restored, String> {
    let handed_over = source == account.dirs.pending();
    let documents = match handed_over {
        true => Documents::new(),
        false => bundle
            .documents_for(&account.name)
            .map_err(|error| error.to_string())?,
    };
    settings_backup::restore(bundle, &account.dirs).map_err(|error| error.to_string())?;
    let mut skipped = Vec::new();
    let mut writes = Vec::new();
    for preference in &bundle.preferences {
        let id = SettingId {
            domain: preference.domain.clone(),
            name: preference.name.clone(),
        };
        match serde_json::from_value::<Value>(preference.value.clone()) {
            Ok(value) => writes.push(Write {
                id,
                value: Some(value),
            }),
            Err(error) => skipped.push(format!("{}/{}: {error}", id.domain, id.name)),
        }
    }
    skipped.extend(Values::connect(seam)?.write_all(writes));
    if handed_over {
        std::fs::remove_file(source)
            .map_err(|error| format!("failed to remove {}: {error}", source.display()))?;
    }
    Ok(Restored { documents, skipped })
}

pub(crate) fn publish(account: &Account, url: &str) -> Result<Published, String> {
    let repository = Repository::new(account.dirs.state.join(CLONE));
    let message = format!("Rikka configuration of {}", host_name()?);
    repository
        .publish(url, &configured()?, &message, &account.identity()?)
        .map_err(|error| error.to_string())
}

pub(crate) fn fetch(account: &Account, url: &str) -> Result<Documents, String> {
    let folder = tempfile::tempdir().map_err(|error| error.to_string())?;
    let configured =
        Repository::fetch(url, &folder.path().join(CHECKOUT)).map_err(|error| error.to_string())?;
    Configured::documents(&configured, None, &account.name).map_err(|error| error.to_string())
}

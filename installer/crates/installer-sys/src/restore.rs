use installer_core::{Answers, Restored, Start};
use rikka_backup::{Bundle, Configured, PORTABLE, clone_arguments};

use crate::{command::Command, host::Host};

const CHECKOUT: &str = "/tmp/rikka-configuration.XXXXXX";

pub fn read_restore(host: &dyn Host, answers: &Answers) -> Result<Restored, String> {
    match answers.start {
        Start::Fresh => Err("a fresh install restores nothing".to_owned()),
        Start::Backup => backup(host, answers),
        Start::Repository => repository(host, answers.repository.trim()),
    }
}

fn backup(host: &dyn Host, answers: &Answers) -> Result<Restored, String> {
    let path = answers.backup_path.trim();
    let sealed = host.read_bytes(path).map_err(|error| error.to_string())?;
    let bundle = Bundle::open(sealed.as_slice(), answers.backup_passphrase.as_str())
        .map_err(|error| error.to_string())?;
    let carried = !bundle.preferences.is_empty()
        || !bundle.files.is_empty()
        || !bundle.credentials.is_empty();
    Ok(Restored {
        sign_ins: bundle.credentials.len(),
        preferences: bundle.preferences.len(),
        pending: carried.then(|| path.to_owned()),
        declared_for: Some(bundle.origin.user),
        modules: bundle.modules,
    })
}

fn repository(host: &dyn Host, url: &str) -> Result<Restored, String> {
    let failed = |error: crate::error::Error| error.to_string();
    let checkout = host
        .capture(&Command::new("mktemp").args(["-d", CHECKOUT]))
        .map_err(failed)?
        .trim()
        .to_owned();
    if checkout.is_empty() {
        return Err("mktemp named no folder".to_owned());
    }
    let clone = clone_arguments(url, &checkout).map_err(|error| error.to_string())?;
    host.run(
        &Command::nix()
            .args(["run", "nixpkgs#git", "--"])
            .args(clone),
    )
    .map_err(failed)?;
    let mut modules = Vec::new();
    for module in PORTABLE {
        let path = format!("{checkout}/{}", module.file_name());
        if host.exists(&path) {
            let text = host.read(&path).map_err(|error| error.to_string())?;
            modules.push(Configured { module, text });
        }
    }
    if modules.is_empty() {
        return Err(format!("{url} holds no Rikka configuration"));
    }
    Ok(Restored {
        modules,
        declared_for: None,
        pending: None,
        sign_ins: 0,
        preferences: 0,
    })
}

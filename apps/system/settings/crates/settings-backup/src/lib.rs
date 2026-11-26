mod carried;
mod config;
mod credentials;
mod dirs;
mod error;
mod preferences;
mod repository;

use std::path::Path;

use rikka_backup::Bundle;
use rikka_fs::Replace;
use rikka_secrets::blocking::Secrets;

pub use config::Config;
pub use dirs::Dirs;
pub use error::{Error, Result};
pub use preferences::preferences;
pub use repository::{Identity, Published, Repository};

pub fn collect_files(dirs: &Dirs) -> Result<Vec<rikka_backup::File>> {
    carried::collect(dirs)
}

pub fn collect_credentials(secrets: &Secrets) -> Result<Vec<rikka_backup::Credential>> {
    credentials::collect(secrets)
}

pub fn export(bundle: &Bundle, passphrase: &str, target: &Path) -> Result<()> {
    write(bundle.seal(passphrase)?, bundle, passphrase, target)?;
    if bundle.credentials.is_empty() {
        return Ok(());
    }
    credentials::forget(&bundle.credentials, &Secrets::shared()?)
}

pub fn restore(bundle: &Bundle, dirs: &Dirs) -> Result<()> {
    carried::restore(bundle, dirs)?;
    if bundle.credentials.is_empty() {
        return Ok(());
    }
    credentials::restore(bundle, &Secrets::shared()?, &dirs.home)
}

fn write(sealed: Vec<u8>, bundle: &Bundle, passphrase: &str, target: &Path) -> Result<()> {
    Replace::new(target)
        .private()
        .write(&sealed)
        .map_err(Error::io(target))?;
    let reread = std::fs::File::open(target).map_err(Error::io(target))?;
    if Bundle::open(reread, passphrase)? != *bundle {
        return Err(Error::Unverified(target.to_owned()));
    }
    Ok(())
}

use std::os::unix::fs::DirBuilderExt;
use std::path::{Component, Path};

use rikka_account_book::FOLDER_ATTRIBUTE;
use rikka_backup::{Bundle, Credential};
use rikka_secrets::blocking::Secrets;
use rikka_secrets::{Entry, schema};
use zeroize::Zeroizing;

use crate::error::{Error, Result};

const PRIVATE_DIR: u32 = 0o700;

pub(crate) fn collect(secrets: &Secrets) -> Result<Vec<Credential>> {
    let mut credentials = Vec::new();
    for schema in schema::ALL {
        for found in secrets.search(&Entry::new(schema))? {
            credentials.push(Credential {
                label: found.label,
                attributes: found.attributes,
                secret: Zeroizing::new(found.secret.expose().to_vec()),
            });
        }
    }
    Ok(credentials)
}

pub(crate) fn restore(bundle: &Bundle, secrets: &Secrets, home: &Path) -> Result<()> {
    let rebase_onto = home
        .to_str()
        .ok_or_else(|| Error::Unnamed(home.to_owned()))?;
    for credential in &bundle.credentials {
        let attributes = credential.rebased(&bundle.origin, rebase_onto);
        if let Some(folder) = attributes.get(FOLDER_ATTRIBUTE)
            && is_inside(home, Path::new(folder))
        {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(PRIVATE_DIR)
                .create(folder)
                .map_err(Error::io(folder))?;
        }
        secrets.store(
            &Entry::from_attributes(attributes),
            &credential.label,
            &credential.secret,
        )?;
    }
    Ok(())
}

fn is_inside(home: &Path, folder: &Path) -> bool {
    folder.strip_prefix(home).is_ok_and(|rest| {
        rest.components()
            .all(|component| matches!(component, Component::Normal(_)))
    })
}

pub(crate) fn forget(credentials: &[Credential], secrets: &Secrets) -> Result<()> {
    for credential in credentials {
        secrets.remove(&Entry::from_attributes(credential.attributes.clone()))?;
    }
    Ok(())
}

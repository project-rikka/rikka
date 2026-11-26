use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;

use rikka_nix_edit::{Document, Expr, Set};
use rikka_nixos_layout::Module;
use serde::{Deserialize, Serialize};

use crate::bundle::PORTABLE;
use crate::error::{Error, Result};

const USERS: [&str; 2] = ["rikka", "users"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configured {
    #[serde(with = "crate::module_name")]
    pub module: Module,
    pub text: String,
}

impl Configured {
    pub fn read_folder(folder: &Path) -> Result<Vec<Configured>> {
        let mut configured = Vec::new();
        for module in PORTABLE {
            let path = folder.join(module.file_name());
            match std::fs::read_to_string(&path) {
                Ok(text) => configured.push(Configured { module, text }),
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(Error::Invalid(format!(
                        "failed to read {}: {error}",
                        path.display()
                    )));
                }
            }
        }
        Ok(configured)
    }

    pub fn documents(
        configured: &[Configured],
        declared_for: Option<&str>,
        user: &str,
    ) -> Result<BTreeMap<Module, Document>> {
        let mut documents = BTreeMap::new();
        for configured in configured {
            let mut document = Document::parse(configured.text.clone())?;
            if configured.module == Module::Settings {
                let from = match declared_for {
                    Some(from) => Some(from.to_owned()),
                    None => only_user(&document)?,
                };
                if let Some(from) = from.filter(|from| from != user) {
                    rename_user(&mut document, &from, user)?;
                }
            }
            documents.insert(configured.module, document);
        }
        Ok(documents)
    }
}

fn only_user(document: &Document) -> Result<Option<String>> {
    let Some(users) = document.get::<Set>(USERS)? else {
        return Ok(None);
    };
    let mut names: Vec<&str> = users
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            rikka_nix_edit::Entry::Binding { path, .. } => {
                path.segments().first().map(String::as_str)
            }
            _ => None,
        })
        .collect();
    names.dedup();
    match names.as_slice() {
        [only] => Ok(Some((*only).to_owned())),
        _ => Ok(None),
    }
}

fn rename_user(document: &mut Document, from: &str, to: &str) -> Result<()> {
    let old = ["rikka", "users", from];
    let Some(declared) = document.get::<Expr>(old)? else {
        return Ok(());
    };
    document.remove(old)?;
    document.set(["rikka", "users", to], declared)?;
    Ok(())
}

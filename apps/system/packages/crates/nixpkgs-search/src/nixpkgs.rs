use nixpkgs_catalog::Package;

use crate::cancel::Cancel;
use crate::error::Result;
use crate::lookup;
use crate::nix::{self, installable};

const BATCH: usize = 200;

#[derive(Clone, Debug)]
pub struct Nixpkgs {
    system: String,
}

impl Nixpkgs {
    pub fn open(system: impl Into<String>, cancel: &Cancel) -> Result<Self> {
        let system = system.into();
        nix::run(
            &[
                "eval",
                &format!("{}.stdenv.hostPlatform.system", installable(&system)),
                "--raw",
            ],
            cancel,
        )?;
        Ok(Self { system })
    }

    pub fn lookup(&self, attributes: &[&str], cancel: &Cancel) -> Result<Vec<Package>> {
        let mut packages = Vec::with_capacity(attributes.len());
        for batch in attributes.chunks(BATCH) {
            packages.extend(lookup::packages(&self.system, batch, cancel)?);
        }
        Ok(packages)
    }
}

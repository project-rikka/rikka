use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use rikka_net::{Checksum, Flow};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::module::{LIBRARY, MODEL_EXTENSION, Module};

const MANIFEST: &str = "https://updates.discord.com/distributions/app/manifests/latest?channel=stable&platform=linux&arch=x64";
const KRISP: &str = "discord_krisp";
const PACKAGE_ROOT: &str = "files";
const BROTLI_BUFFER: usize = 1 << 16;

#[derive(Deserialize)]
struct Manifest {
    modules: std::collections::HashMap<String, Package>,
}

#[derive(Deserialize)]
struct Package {
    full: Build,
}

#[derive(Deserialize)]
struct Build {
    host_version: Vec<u32>,
    module_version: u32,
    package_sha256: String,
    url: String,
}

impl Build {
    fn release(&self) -> String {
        let host: Vec<String> = self.host_version.iter().map(u32::to_string).collect();
        format!("{}-{}", host.join("."), self.module_version)
    }
}

impl Module {
    pub fn fetch(
        cache: &Path,
        mut progress: impl FnMut(u64, Option<u64>) + Send + 'static,
    ) -> Result<Module> {
        let client = rikka_net::blocking::Client::new();
        let manifest: Manifest = client.get_json(MANIFEST).map_err(download)?;
        let build = manifest
            .modules
            .get(KRISP)
            .map(|package| &package.full)
            .ok_or_else(|| Error::Download("Discord's manifest lists no Krisp module".into()))?;
        let folder = cache.join(build.release());
        if let Ok(module) = Module::from_folder(&folder) {
            return Ok(module);
        }
        std::fs::create_dir_all(cache)?;
        let package = cache.join(format!(".{}.distro", build.release()));
        let checksum = Checksum::sha256(&build.package_sha256).map_err(download)?;
        client
            .download(build.url.as_str(), &package)
            .resume(true)
            .checksum(checksum)
            .on_progress(move |transfer| {
                progress(transfer.transferred, transfer.total);
                Flow::Continue
            })
            .spawn()
            .join()
            .map_err(download)?;
        let unpacking = cache.join(format!(".{}.unpacking", build.release()));
        let unpacked = unpack(&package, &unpacking).and_then(|()| Module::from_folder(&unpacking));
        let _ = std::fs::remove_file(&package);
        if let Err(error) = unpacked {
            let _ = std::fs::remove_dir_all(&unpacking);
            return Err(error);
        }
        if folder.exists() {
            std::fs::remove_dir_all(&folder)?;
        }
        std::fs::rename(&unpacking, &folder)?;
        Module::from_folder(folder)
    }
}

fn unpack(package: &Path, into: &Path) -> Result<()> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    std::fs::create_dir_all(into)?;
    let compressed = BufReader::new(File::open(package)?);
    let mut archive = tar::Archive::new(brotli_decompressor::Decompressor::new(
        compressed,
        BROTLI_BUFFER,
    ));
    let mut extracted = 0;
    for entry in archive.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(unreadable)?.into_owned();
        let Some(name) = extracted_name(&path) else {
            continue;
        };
        let mut file = File::create(into.join(name))?;
        std::io::copy(&mut entry, &mut file)?;
        extracted += 1;
    }
    if extracted == 0 {
        return Err(Error::Unpack("the package holds no Krisp files".into()));
    }
    Ok(())
}

fn extracted_name(path: &Path) -> Option<PathBuf> {
    let mut parts = path.components();
    let root = parts.next()?.as_os_str();
    let name = parts.next()?.as_os_str().to_str()?;
    let flat = parts.next().is_none() && root == PACKAGE_ROOT;
    let is_payload = name == LIBRARY || name.ends_with(MODEL_EXTENSION);
    (flat && is_payload).then(|| PathBuf::from(name))
}

fn download(error: rikka_net::Error) -> Error {
    Error::Download(error.to_string())
}

fn unreadable(error: std::io::Error) -> Error {
    Error::Unpack(error.to_string())
}

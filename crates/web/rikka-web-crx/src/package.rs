use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::error::{Error, Result};
use crate::identity::Identity;
use crate::proto;

const MANIFEST: &str = "manifest.json";

const MAGIC: &[u8] = b"Cr24";

const FORMAT: u32 = 3;

const SIGNATURE_CONTEXT: &[u8] = b"CRX3 SignedData\x00";

const ECDSA_PROOFS: u64 = 3;

const SIGNED_HEADER: u64 = 10000;

const CRX_ID: u64 = 1;

const PUBLIC_KEY: u64 = 1;

const SIGNATURE: u64 = 2;

const END_OF_DIRECTORY: &[u8] = b"PK\x05\x06";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version([u16; 4]);

impl Version {
    pub fn parse(text: &str) -> Result<Version> {
        let parts: Vec<u16> = text
            .split('.')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::Version(text.to_owned()))?;
        if parts.is_empty() || parts.len() > 4 {
            return Err(Error::Version(text.to_owned()));
        }
        let mut version = [0; 4];
        version[..parts.len()].copy_from_slice(&parts);
        Ok(Version(version))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [major, minor, build, patch] = self.0;
        write!(formatter, "{major}.{minor}.{build}.{patch}")
    }
}

#[derive(Clone, Debug, Default)]
pub struct Package {
    files: BTreeMap<String, Vec<u8>>,
}

impl Package {
    #[must_use]
    pub fn new() -> Package {
        Package::default()
    }

    #[must_use]
    pub fn file(mut self, path: impl Into<String>, contents: Vec<u8>) -> Package {
        self.files.insert(path.into(), contents);
        self
    }

    pub fn from_zip(bytes: &[u8]) -> Result<Package> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))?;
        let mut files = BTreeMap::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            if entry.is_dir() {
                continue;
            }
            if entry.enclosed_name().is_none() {
                return Err(Error::UnsafePath(entry.name().to_owned()));
            }
            let mut contents = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut contents)?;
            files.insert(entry.name().to_owned(), contents);
        }
        Ok(Package { files })
    }

    pub fn to_zip(&self) -> Result<Vec<u8>> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(DateTime::default())
            .unix_permissions(0o644);
        for (path, contents) in &self.files {
            writer.start_file(path.as_str(), options)?;
            writer.write_all(contents)?;
        }
        Ok(writer.finish()?.into_inner())
    }

    pub fn version(&self) -> Result<Version> {
        let manifest = self.manifest()?;
        let version = manifest
            .get("version")
            .and_then(serde_json::Value::as_str)
            .ok_or(Error::ManifestShape)?;
        Version::parse(version)
    }

    pub fn stamp(mut self, identity: &Identity, version: Version) -> Result<Package> {
        let mut manifest = self.manifest()?;
        manifest.insert("key".into(), identity.manifest_key().into());
        manifest.insert("version".into(), version.to_string().into());
        let encoded = serde_json::to_vec_pretty(&serde_json::Value::Object(manifest))?;
        self.files.insert(MANIFEST.into(), encoded);
        Ok(self)
    }

    pub fn crx(&self, identity: &Identity) -> Result<Vec<u8>> {
        let archive = self.to_zip()?;
        let mut signed_data = Vec::new();
        proto::bytes_field(CRX_ID, &identity.crx_id(), &mut signed_data);
        let signed_length = u32::try_from(signed_data.len()).expect("the signed header is tiny");
        let mut message = Vec::with_capacity(SIGNATURE_CONTEXT.len() + 4 + signed_data.len());
        message.extend_from_slice(SIGNATURE_CONTEXT);
        message.extend_from_slice(&signed_length.to_le_bytes());
        message.extend_from_slice(&signed_data);
        message.extend_from_slice(&archive);
        let mut proof = Vec::new();
        proto::bytes_field(PUBLIC_KEY, identity.public_key(), &mut proof);
        proto::bytes_field(SIGNATURE, &identity.sign(&message), &mut proof);
        let mut header = Vec::new();
        proto::bytes_field(ECDSA_PROOFS, &proof, &mut header);
        proto::bytes_field(SIGNED_HEADER, &signed_data, &mut header);
        if header
            .windows(END_OF_DIRECTORY.len())
            .any(|window| window == END_OF_DIRECTORY)
        {
            return Err(Error::HeaderMarker);
        }
        let header_length = u32::try_from(header.len()).expect("the header is tiny");
        let mut crx = Vec::with_capacity(12 + header.len() + archive.len());
        crx.extend_from_slice(MAGIC);
        crx.extend_from_slice(&FORMAT.to_le_bytes());
        crx.extend_from_slice(&header_length.to_le_bytes());
        crx.extend_from_slice(&header);
        crx.extend_from_slice(&archive);
        Ok(crx)
    }

    pub fn unpack(&self, folder: &Path) -> Result<()> {
        for (path, contents) in &self.files {
            let relative = Path::new(path);
            if !relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
            {
                return Err(Error::UnsafePath(path.clone()));
            }
            let target = folder.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, contents)?;
        }
        Ok(())
    }

    fn manifest(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        let bytes = self.files.get(MANIFEST).ok_or(Error::MissingManifest)?;
        match serde_json::from_slice(bytes)? {
            serde_json::Value::Object(manifest) => Ok(manifest),
            _ => Err(Error::ManifestShape),
        }
    }
}

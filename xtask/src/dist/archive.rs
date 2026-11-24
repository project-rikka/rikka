use std::fs::File;
use std::io::{self, BufWriter, Read as _, Write};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

use crate::dist::release::Packed;

const LEVEL: i32 = 19;

const EXECUTABLE: u32 = 0o111;

const DIGEST_LENGTH: usize = 16;

pub(crate) const ARCHIVE_SUFFIX: &str = ".tar.zst";

struct Archived {
    packed: Packed,
    digest: String,
}

pub(crate) fn pack_named(tree: &Path, top: &str, stem: &str, out: &Path) -> Result<Packed> {
    let unnamed = out.join(format!("{stem}{ARCHIVE_SUFFIX}.new"));
    let archived = pack(tree, top, &unnamed)?;
    let file = format!(
        "{stem}-{}{ARCHIVE_SUFFIX}",
        &archived.digest[..DIGEST_LENGTH]
    );
    let named = out.join(&file);
    std::fs::rename(&unnamed, &named).with_context(|| {
        format!(
            "failed to move {} to {}",
            unnamed.display(),
            named.display()
        )
    })?;
    Ok(Packed {
        file,
        hash: archived.packed.hash,
    })
}

pub(crate) fn pack_as(tree: &Path, top: &str, file: &str, out: &Path) -> Result<Packed> {
    Ok(pack(tree, top, &out.join(file))?.packed)
}

fn pack(tree: &Path, top: &str, file: &Path) -> Result<Archived> {
    let output =
        File::create(file).with_context(|| format!("failed to create {}", file.display()))?;
    let mut encoder = zstd::Encoder::new(BufWriter::new(output), LEVEL)
        .with_context(|| format!("failed to start compressing {}", file.display()))?;
    encoder
        .include_checksum(true)
        .context("failed to configure zstd")?;
    let mut hashing = Hashing {
        inner: encoder,
        hasher: Sha256::new(),
    };
    {
        let mut builder = tar::Builder::new(&mut hashing);
        builder.follow_symlinks(false);
        builder.mode(tar::HeaderMode::Deterministic);
        append(&mut builder, tree, Path::new(top))?;
        builder
            .finish()
            .with_context(|| format!("failed to write {}", file.display()))?;
    }
    let digest = hex(&hashing.hasher.finalize());
    hashing
        .inner
        .finish()
        .and_then(|mut written| written.flush())
        .with_context(|| format!("failed to finish {}", file.display()))?;
    Ok(Archived {
        packed: Packed {
            file: name_of(file)?,
            hash: sri(file)?,
        },
        digest,
    })
}

pub(crate) fn sri(file: &Path) -> Result<String> {
    Ok(format!("sha256-{}", STANDARD.encode(sha256(file)?)))
}

pub(crate) fn sha256_hex(file: &Path) -> Result<String> {
    Ok(hex(&sha256(file)?))
}

fn sha256(file: &Path) -> Result<Vec<u8>> {
    let mut hasher = Sha256::new();
    let mut input =
        File::open(file).with_context(|| format!("failed to open {}", file.display()))?;
    let mut buffer = vec![0; 1 << 20];
    loop {
        let read = input
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", file.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_vec())
}

fn append<W: Write>(builder: &mut tar::Builder<W>, directory: &Path, inside: &Path) -> Result<()> {
    builder
        .append_dir(inside, directory)
        .with_context(|| format!("failed to archive {}", directory.display()))?;
    for entry in sorted(directory)? {
        let named = inside.join(entry.file_name().context("listed entries have names")?);
        let metadata = std::fs::symlink_metadata(&entry)
            .with_context(|| format!("failed to read {}", entry.display()))?;
        if metadata.is_dir() {
            append(builder, &entry, &named)?;
            continue;
        }
        if metadata.file_type().is_symlink() {
            let target = std::fs::read_link(&entry)
                .with_context(|| format!("failed to read link {}", entry.display()))?;
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            builder
                .append_link(&mut header, &named, &target)
                .with_context(|| format!("failed to archive {}", entry.display()))?;
            continue;
        }
        let mode = match metadata.permissions().mode() & EXECUTABLE {
            0 => 0o644,
            _ => 0o755,
        };
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(metadata.len());
        header.set_mode(mode);
        let mut contents =
            File::open(&entry).with_context(|| format!("failed to open {}", entry.display()))?;
        builder
            .append_data(&mut header, &named, &mut contents)
            .with_context(|| format!("failed to archive {}", entry.display()))?;
    }
    Ok(())
}

fn sorted(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed to list {}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()
        .with_context(|| format!("failed to list {}", directory.display()))?;
    entries.sort();
    Ok(entries)
}

fn name_of(file: &Path) -> Result<String> {
    file.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("{} names no file", file.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct Hashing<W> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.hasher.update(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

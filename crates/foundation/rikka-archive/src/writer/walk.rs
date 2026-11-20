use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::Result;
use crate::path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceKind {
    File { size: u64 },
    Directory,
    Symlink { target: String },
}

#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) kind: SourceKind,
    pub(crate) mode: Option<u32>,
    pub(crate) modified: Option<SystemTime>,
}

impl Source {
    pub(crate) fn size(&self) -> u64 {
        match self.kind {
            SourceKind::File { size } => size,
            _ => 0,
        }
    }
}

pub(crate) fn collect(path: &Path, name: &str, follow_symlinks: bool) -> Result<Vec<Source>> {
    let mut sources = Vec::new();
    visit(path, &path::normalize(name), follow_symlinks, &mut sources)?;
    Ok(sources)
}

fn visit(path: &Path, name: &str, follow_symlinks: bool, sources: &mut Vec<Source>) -> Result<()> {
    let metadata = if follow_symlinks {
        std::fs::metadata(path)?
    } else {
        std::fs::symlink_metadata(path)?
    };

    if metadata.is_symlink() {
        let target = std::fs::read_link(path)?;
        sources.push(source(
            path,
            name.trim_end_matches('/').to_owned(),
            SourceKind::Symlink {
                target: target.to_string_lossy().replace('\\', "/"),
            },
            &metadata,
        ));
        return Ok(());
    }

    if metadata.is_file() {
        sources.push(source(
            path,
            name.trim_end_matches('/').to_owned(),
            SourceKind::File {
                size: metadata.len(),
            },
            &metadata,
        ));
        return Ok(());
    }

    let directory_name = format!("{}/", name.trim_end_matches('/'));
    sources.push(source(
        path,
        directory_name.clone(),
        SourceKind::Directory,
        &metadata,
    ));

    let mut children = std::fs::read_dir(path)?
        .map(|child| child.map(|child| child.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    children.sort();

    for child in children {
        let child_name = format!("{directory_name}{}", child.to_string_lossy());
        visit(&path.join(&child), &child_name, follow_symlinks, sources)?;
    }
    Ok(())
}

fn source(path: &Path, name: String, kind: SourceKind, metadata: &Metadata) -> Source {
    Source {
        path: path.to_owned(),
        name,
        kind,
        mode: mode_of(metadata),
        modified: metadata.modified().ok(),
    }
}

fn mode_of(metadata: &Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;

    Some(metadata.permissions().mode())
}

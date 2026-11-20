use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub(crate) const LIBRARY: &str = "discord_krisp.node";
pub(crate) const MODEL_EXTENSION: &str = ".kef";
const NOISE_PREFIX: &str = "krisp-nc-";
const VOICE_PREFIX: &str = "krisp-vad-";
const NARROW_TIERS: [&str; 2] = ["-nb-", "-lite-"];
const FULL_TIER: &str = "-med-";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    folder: PathBuf,
    noise_model: PathBuf,
    voice_model: PathBuf,
}

impl Module {
    pub fn from_folder(folder: impl Into<PathBuf>) -> Result<Module> {
        let folder = folder.into();
        let incomplete = |missing: &str| Error::Incomplete {
            folder: folder.clone(),
            missing: missing.to_owned(),
        };
        if !folder.join(LIBRARY).is_file() {
            return Err(incomplete(LIBRARY));
        }
        let names = model_names(&folder);
        let noise_model = noise_model(&names).ok_or_else(|| incomplete("a noise model"))?;
        let voice_model =
            newest(&names, VOICE_PREFIX).ok_or_else(|| incomplete("a voice activity model"))?;
        Ok(Module {
            noise_model: folder.join(noise_model),
            voice_model: folder.join(voice_model),
            folder,
        })
    }

    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub(crate) fn library(&self) -> PathBuf {
        self.folder.join(LIBRARY)
    }

    #[must_use]
    pub fn noise_model(&self) -> &Path {
        &self.noise_model
    }

    #[must_use]
    pub fn voice_model(&self) -> &Path {
        &self.voice_model
    }
}

fn model_names(folder: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(MODEL_EXTENSION))
        .collect()
}

fn noise_model(names: &[String]) -> Option<&String> {
    let wide: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with(NOISE_PREFIX))
        .filter(|name| !NARROW_TIERS.iter().any(|tier| name.contains(tier)))
        .collect();
    let full: Vec<&String> = wide
        .iter()
        .copied()
        .filter(|name| name.contains(FULL_TIER))
        .collect();
    let pool = if full.is_empty() { wide } else { full };
    pool.into_iter().max_by_key(|name| version(name))
}

fn newest<'a>(names: &'a [String], prefix: &str) -> Option<&'a String> {
    names
        .iter()
        .filter(|name| name.starts_with(prefix))
        .max_by_key(|name| version(name))
}

pub(crate) fn version(name: &str) -> Vec<u64> {
    name.split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

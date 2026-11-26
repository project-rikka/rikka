use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_sync::Mutex;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::folder::entries;
use crate::instance::{CONFIG, Instance};

const FALLBACK: &str = "instance";
const MAX_SLUG: usize = 48;

#[derive(Clone, Debug)]
pub struct Instances {
    root: PathBuf,
    writing: Arc<Mutex<()>>,
}

#[derive(Debug, Default)]
pub struct Listing {
    pub instances: Vec<Instance>,
    pub problems: Vec<Error>,
}

fn slug(name: &str) -> String {
    let mut slug = String::new();
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= MAX_SLUG {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    match slug.is_empty() {
        true => FALLBACK.to_owned(),
        false => slug.to_owned(),
    }
}

impl Instances {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Instances {
        Instances {
            root: root.into(),
            writing: Arc::default(),
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn list(&self) -> Result<Listing> {
        let mut listing = Listing::default();
        for entry in entries(&self.root)? {
            let path = entry.path();
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !path.join(CONFIG).is_file() {
                continue;
            }
            match Instance::load(id, path) {
                Ok(instance) => listing.instances.push(instance),
                Err(error) => listing.problems.push(error),
            }
        }
        listing.instances.sort_by(|left, right| {
            right
                .config
                .last_played
                .cmp(&left.config.last_played)
                .then_with(|| right.config.created.cmp(&left.config.created))
        });
        Ok(listing)
    }

    pub fn get(&self, id: &str) -> Result<Instance> {
        let path = self.root.join(id);
        if id.contains(['/', '\\']) || id.starts_with('.') || !path.join(CONFIG).is_file() {
            return Err(Error::NoInstance(id.to_owned()));
        }
        Instance::load(id.to_owned(), path)
    }

    pub fn update(&self, id: &str, change: impl FnOnce(&mut Config)) -> Result<Instance> {
        let _writing = self.writing.lock();
        let mut instance = self.get(id)?;
        change(&mut instance.config);
        instance.save()?;
        Ok(instance)
    }

    pub fn create(&self, name: &str) -> Result<Instance> {
        fs::create_dir_all(&self.root).map_err(Error::io(&self.root))?;
        let base = slug(name);
        let mut attempt = 1;
        loop {
            let id = match attempt {
                1 => base.clone(),
                number => format!("{base}-{number}"),
            };
            let path = self.root.join(&id);
            match fs::create_dir(&path) {
                Ok(()) => {
                    let instance = Instance::new(id, path, Config::new(name.trim()));
                    instance.save()?;
                    return Ok(instance);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => attempt += 1,
                Err(source) => return Err(Error::Io { path, source }),
            }
        }
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let instance = self.get(id)?;
        fs::remove_dir_all(&instance.path).map_err(Error::io(&instance.path))
    }
}

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rikka_dirs::Paths;

use crate::builtin::{builtin_themes, default_theme};
use crate::error::{Error, Result};
use crate::material::MaterialYou;
use crate::spec::ThemeSpec;

#[derive(Clone, Debug, Default)]
#[must_use]
pub struct ThemeRegistry {
    themes: BTreeMap<String, ThemeSpec>,
}

#[derive(Debug, Default)]
pub struct LoadReport {
    pub loaded: Vec<String>,
    pub errors: Vec<LoadError>,
}

#[derive(Debug)]
pub struct LoadError {
    pub path: PathBuf,
    pub error: Error,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn embedded() -> Self {
        let mut registry = Self::new();
        for spec in builtin_themes() {
            registry.insert(spec.clone());
        }
        registry
    }

    pub fn discover(material: MaterialYou) -> Result<Self> {
        let mut registry = Self::embedded();
        for spec in material.specs() {
            registry.insert(spec);
        }
        for directory in Paths::resolve()?.themes().into_iter().rev() {
            let report = registry.load_dir(directory)?;
            for LoadError { path, error } in &report.errors {
                tracing::warn!(path = %path.display(), %error, "skipping invalid theme");
            }
        }
        Ok(registry)
    }

    pub fn resolve(slug: &str, material: MaterialYou) -> Result<Option<ThemeSpec>> {
        for directory in Paths::resolve()?.themes() {
            for path in theme_files(&directory)?.into_iter().rev() {
                match ThemeSpec::from_path(&path) {
                    Ok(spec) if spec.slug == slug => return Ok(Some(spec)),
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(path = %path.display(), %error, "skipping invalid theme");
                    }
                }
            }
        }
        Ok(material.named(slug).or_else(|| {
            builtin_themes()
                .iter()
                .find(|spec| spec.slug == slug)
                .cloned()
        }))
    }

    #[must_use]
    pub fn named(slug: &str, material: MaterialYou) -> ThemeSpec {
        if slug.is_empty() {
            return default_theme().clone();
        }
        match Self::resolve(slug, material) {
            Ok(Some(spec)) => return spec,
            Ok(None) => tracing::warn!("unknown theme `{slug}`, using the default"),
            Err(error) => tracing::warn!("failed to read themes: {error}"),
        }
        default_theme().clone()
    }

    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> Result<LoadReport> {
        let mut report = LoadReport::default();
        for path in theme_files(dir.as_ref())? {
            match ThemeSpec::from_path(&path) {
                Ok(spec) => {
                    report.loaded.push(spec.slug.clone());
                    self.insert(spec);
                }
                Err(error) => report.errors.push(LoadError { path, error }),
            }
        }
        Ok(report)
    }

    pub fn insert(&mut self, spec: ThemeSpec) -> Option<ThemeSpec> {
        self.themes.insert(spec.slug.clone(), spec)
    }

    pub fn get(&self, slug: &str) -> Option<&ThemeSpec> {
        self.themes.get(slug)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ThemeSpec> {
        self.themes.values()
    }

    pub fn len(&self) -> usize {
        self.themes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }
}

fn theme_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(Error::ReadDir {
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(io::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        })
        .collect();
    paths.sort();
    Ok(paths)
}

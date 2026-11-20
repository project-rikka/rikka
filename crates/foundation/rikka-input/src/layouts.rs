use std::ffi::OsStr;
use std::path::PathBuf;

use crate::error::{Error, Result};

const ROOT_ENV: &str = "XKB_CONFIG_ROOT";
const ROOT: &str = "/run/current-system/sw/share/X11/xkb";
const LISTS: &[&str] = &["evdev.lst", "base.lst"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub name: String,
    pub description: String,
    pub variants: Vec<Variant>,
}

impl Layout {
    #[must_use]
    pub fn variant(&self, name: &str) -> Option<&Variant> {
        self.variants.iter().find(|variant| variant.name == name)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Layouts {
    layouts: Vec<Layout>,
}

impl Layouts {
    pub fn load() -> Result<Self> {
        let path = list_path().ok_or_else(|| Error::LayoutList(describe_roots()))?;
        let text = std::fs::read_to_string(&path)
            .map_err(|error| Error::LayoutList(format!("{}: {error}", path.display())))?;
        Ok(Self::parse(&text))
    }

    #[must_use]
    pub fn parse(list: &str) -> Self {
        let mut layouts: Vec<Layout> = Vec::new();
        let mut section = "";
        for line in list.lines() {
            if let Some(name) = line.strip_prefix('!') {
                section = name.trim();
                continue;
            }
            let Some((name, rest)) = split(line) else {
                continue;
            };
            match section {
                "layout" => layouts.push(Layout {
                    name: name.to_owned(),
                    description: rest.to_owned(),
                    variants: Vec::new(),
                }),
                "variant" => {
                    let Some((owner, description)) = rest.split_once(':') else {
                        continue;
                    };
                    if let Some(layout) = layouts
                        .iter_mut()
                        .find(|layout| layout.name == owner.trim())
                    {
                        layout.variants.push(Variant {
                            name: name.to_owned(),
                            description: description.trim().to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }
        Self { layouts }
    }

    #[must_use]
    pub fn all(&self) -> &[Layout] {
        &self.layouts
    }

    #[must_use]
    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.layouts.iter().find(|layout| layout.name == name)
    }

    #[must_use]
    pub fn describe(&self, layout: &str, variant: &str) -> Option<String> {
        let layout = self.layout(layout)?;
        if variant.is_empty() {
            return Some(layout.description.clone());
        }
        layout
            .variant(variant)
            .map(|variant| variant.description.clone())
    }
}

fn split(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (name, rest) = line.split_once(char::is_whitespace)?;
    let rest = rest.trim();
    (!name.is_empty() && !rest.is_empty()).then_some((name, rest))
}

fn xkb_roots(named: Option<&OsStr>) -> Vec<PathBuf> {
    named
        .map(PathBuf::from)
        .filter(|root| root.is_absolute())
        .into_iter()
        .chain(std::iter::once(PathBuf::from(ROOT)))
        .collect()
}

fn configured_roots() -> Vec<PathBuf> {
    xkb_roots(std::env::var_os(ROOT_ENV).as_deref())
}

fn list_path() -> Option<PathBuf> {
    configured_roots()
        .into_iter()
        .flat_map(|root| LISTS.iter().map(move |list| root.join("rules").join(list)))
        .find(|path| path.is_file())
}

fn describe_roots() -> String {
    configured_roots()
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

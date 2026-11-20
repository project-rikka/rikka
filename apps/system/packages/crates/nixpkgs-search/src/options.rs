use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use serde::de::{IgnoredAny, MapAccess, Visitor};

use crate::cancel::Cancel;
use crate::error::{Error, Result};
use crate::nix::{self, installable};
use crate::recipe::{self, RecipeItem};
use crate::requirement::{Requirement, Setting, Suggestion};

const INDEX: &str = "share/doc/nixos/options.json";

const ENABLE: &str = ".enable";
const PACKAGE: &str = ".package";

const SCOPE: &str = "pkgs.";

const PROGRAMS: &str = "programs.";

const SCOPES: [&str; 2] = [PROGRAMS, "services."];

#[derive(Clone, Debug)]
pub struct Options {
    modules_by_package: HashMap<String, Vec<String>>,
    enables: HashMap<String, String>,
    recipe_options: HashMap<String, String>,
    installs: HashMap<String, String>,
}

impl Options {
    pub fn open(system: &str, cancel: &Cancel) -> Result<Self> {
        let source = nix::run(
            &["eval", &format!("{}.path", installable(system)), "--raw"],
            cancel,
        )?;
        let source = String::from_utf8_lossy(&source).trim().to_owned();
        if source.is_empty() {
            return Err(Error::Unexpected("nix eval returned no nixpkgs path"));
        }

        let out_paths = nix::run(
            &[
                "build",
                "--file",
                &format!("{source}/nixos/lib/eval-config.nix"),
                "--argstr",
                "system",
                system,
                "--arg",
                "modules",
                "[ ]",
                "--no-link",
                "--print-out-paths",
                "config.system.build.manual.optionsJSON",
            ],
            cancel,
        )?;

        let out_paths = String::from_utf8_lossy(&out_paths);
        let path = out_paths
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .ok_or(Error::Unexpected("nix build returned no options.json path"))?;

        let index_path = Path::new(path).join(INDEX);
        let read = std::fs::read(&index_path).map_err(|source| Error::Read {
            path: index_path.clone(),
            source,
        })?;
        Self::read(&read)
    }

    fn read(index: &[u8]) -> Result<Self> {
        let read: RawOptions = serde_json::from_slice(index)?;
        Ok(read.into_options())
    }

    #[must_use]
    pub fn requirements(&self, attribute: &str, unfree: bool) -> Vec<Suggestion> {
        let mut suggestions = self.recipe_requirements(attribute);
        if unfree {
            suggestions.insert(
                Requirement::Unfree.key(),
                Suggestion {
                    requirement: Requirement::Unfree,
                    recommended: true,
                },
            );
        }
        sorted(suggestions)
    }

    #[must_use]
    pub fn installed_package(&self, path: &str) -> Option<&str> {
        self.installs.get(path).map(String::as_str)
    }

    fn recipe_requirements(&self, attribute: &str) -> BTreeMap<String, Suggestion> {
        let mut suggestions = BTreeMap::new();
        let modules = self
            .modules_by_package
            .get(attribute)
            .map_or(&[][..], Vec::as_slice);
        let owner = owner(modules, attribute);

        for base in modules {
            let recommended = owner == Some(base.as_str()) && base.starts_with(PROGRAMS);
            self.add_enable(&mut suggestions, base, recommended);
        }
        if modules.is_empty() {
            for scope in SCOPES {
                self.add_enable(&mut suggestions, &format!("{scope}{attribute}"), false);
            }
        }

        for want in recipe::items_for(attribute) {
            let (requirement, recommended) = match want {
                RecipeItem::Enable(path, recommended) => {
                    (self.recipe_option(path, Setting::Enabled), *recommended)
                }
                RecipeItem::Value(path, word, recommended) => (
                    self.recipe_option(path, Setting::Value((*word).to_owned())),
                    *recommended,
                ),
                RecipeItem::Group(name, recommended) => {
                    (Some(Requirement::Group((*name).to_owned())), *recommended)
                }
            };
            if let Some(requirement) = requirement {
                suggestions.insert(
                    requirement.key(),
                    Suggestion {
                        requirement,
                        recommended,
                    },
                );
            }
        }
        suggestions
    }

    fn add_enable(
        &self,
        suggestions: &mut BTreeMap<String, Suggestion>,
        base: &str,
        recommended: bool,
    ) {
        let Some(description) = self.enables.get(base) else {
            return;
        };
        let requirement = Requirement::Option {
            path: format!("{base}{ENABLE}"),
            setting: Setting::Enabled,
            description: description.clone(),
        };
        suggestions.insert(
            requirement.key(),
            Suggestion {
                requirement,
                recommended,
            },
        );
    }

    fn recipe_option(&self, path: &str, setting: Setting) -> Option<Requirement> {
        self.recipe_options
            .get(path)
            .map(|description| Requirement::Option {
                path: path.to_owned(),
                setting,
                description: description.clone(),
            })
    }
}

fn owner<'a>(bases: &'a [String], attribute: &str) -> Option<&'a str> {
    let whole = |base: &str| {
        bases
            .iter()
            .all(|other| other == base || other.starts_with(&format!("{base}.")))
    };
    if let Some(base) = bases.iter().map(String::as_str).find(|base| whole(base)) {
        return Some(base);
    }

    let named: Vec<&str> = bases
        .iter()
        .map(String::as_str)
        .filter(|base| {
            SCOPES
                .iter()
                .any(|scope| base.strip_prefix(scope) == Some(attribute))
        })
        .collect();
    match named.as_slice() {
        [base] => Some(base),
        _ => None,
    }
}

fn sorted(suggestions: BTreeMap<String, Suggestion>) -> Vec<Suggestion> {
    let mut listed: Vec<Suggestion> = suggestions.into_values().collect();
    listed.sort_by(|one, two| {
        two.recommended
            .cmp(&one.recommended)
            .then_with(|| one.requirement.key().cmp(&two.requirement.key()))
    });
    listed
}

#[derive(Deserialize)]
struct Entry {
    #[serde(default)]
    description: String,
    #[serde(default)]
    default: Option<serde_json::Value>,
}

#[derive(Default)]
struct RawOptions {
    enables: HashMap<String, String>,
    owners: Vec<(String, String)>,
    recipe_options: HashMap<String, String>,
}

impl RawOptions {
    fn into_options(self) -> Options {
        let mut modules_by_package: HashMap<String, Vec<String>> = HashMap::new();
        let mut installs = HashMap::new();
        for (attribute, base) in self.owners {
            if !self.enables.contains_key(&base) {
                continue;
            }
            if base.starts_with(PROGRAMS) {
                installs.insert(format!("{base}{ENABLE}"), attribute.clone());
            }
            modules_by_package.entry(attribute).or_default().push(base);
        }
        for bases in modules_by_package.values_mut() {
            bases.sort();
        }
        Options {
            modules_by_package,
            enables: self.enables,
            recipe_options: self.recipe_options,
            installs,
        }
    }
}

impl<'de> Deserialize<'de> for RawOptions {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(RawOptionsVisitor)
    }
}

struct RawOptionsVisitor;

impl<'de> Visitor<'de> for RawOptionsVisitor {
    type Value = RawOptions;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the NixOS options.json")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<RawOptions, M::Error> {
        let recipe_paths: HashSet<&str> = recipe::paths().collect();
        let mut read = RawOptions::default();

        while let Some(path) = map.next_key::<String>()? {
            let in_recipe = recipe_paths.contains(path.as_str());
            if !in_recipe && !path.ends_with(ENABLE) && !path.ends_with(PACKAGE) {
                map.next_value::<IgnoredAny>()?;
                continue;
            }

            let entry: Entry = map.next_value()?;
            if in_recipe {
                read.recipe_options
                    .insert(path.clone(), entry.description.clone());
            }
            if let Some(base) = path.strip_suffix(ENABLE) {
                read.enables.insert(base.to_owned(), entry.description);
            } else if let Some(base) = path.strip_suffix(PACKAGE)
                && let Some(attribute) = attribute(&entry)
            {
                read.owners.push((attribute, base.to_owned()));
            }
        }
        Ok(read)
    }
}

fn attribute(entry: &Entry) -> Option<String> {
    let text = entry.default.as_ref()?.get("text")?.as_str()?;
    let attribute = text.trim().strip_prefix(SCOPE)?;
    let named = |ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.');
    if attribute.is_empty()
        || attribute.starts_with('.')
        || attribute.ends_with('.')
        || !attribute.chars().all(named)
    {
        return None;
    }
    Some(attribute.to_owned())
}

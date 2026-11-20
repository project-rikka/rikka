use std::path::{Path, PathBuf};

use rikka_xml::{Element, parse_lenient};

use crate::config::environment::Environment;
use crate::config::fontconfig::Fontconfig;
use crate::config::glob::Glob;
use crate::report::LoadProblem;

const MAX_INCLUDE_DEPTH: usize = 16;
const CONFIG_EXTENSION: &str = ".conf";

pub(crate) struct Loader<'a> {
    environment: &'a Environment,
    problems: &'a mut Vec<LoadProblem>,
    config: Fontconfig,
    active: Vec<PathBuf>,
}

#[derive(Clone, Copy)]
enum Base {
    Data,
    Config,
}

impl<'a> Loader<'a> {
    pub(crate) fn new(environment: &'a Environment, problems: &'a mut Vec<LoadProblem>) -> Self {
        Self {
            environment,
            problems,
            config: Fontconfig::default(),
            active: Vec::new(),
        }
    }

    pub(crate) fn load_main(&mut self, file: &Path) {
        if file.is_file() {
            self.load_file(file);
        } else {
            self.problems
                .push(LoadProblem::MissingConfig(file.to_owned()));
        }
    }

    pub(crate) fn finish(self) -> Fontconfig {
        self.config
    }

    fn load_file(&mut self, file: &Path) {
        if self.active.len() >= MAX_INCLUDE_DEPTH || self.active.iter().any(|open| open == file) {
            return;
        }
        let source = match std::fs::read(file) {
            Ok(source) => source,
            Err(source) => {
                self.problems.push(LoadProblem::UnreadableConfig {
                    path: file.to_owned(),
                    source,
                });
                return;
            }
        };
        let text = String::from_utf8_lossy(&source);
        let Some(root) = parse_lenient(&text).filter(|root| root.name() == "fontconfig") else {
            self.problems
                .push(LoadProblem::MalformedConfig(file.to_owned()));
            return;
        };
        self.active.push(file.to_owned());
        for element in root.children() {
            self.apply(element, file);
        }
        self.active.pop();
    }

    fn apply(&mut self, element: &Element<'_>, file: &Path) {
        match element.name() {
            "dir" => {
                if let Some(dir) = self.resolve(element, Base::Data, file) {
                    self.config.dirs.push(dir);
                }
            }
            "reset-dirs" => self.config.dirs.clear(),
            "include" => self.include(element, file),
            "selectfont" => self.select_fonts(element),
            "alias" => self.config.aliases.add(element),
            _ => {}
        }
    }

    fn resolve(&self, element: &Element<'_>, base: Base, file: &Path) -> Option<PathBuf> {
        let value = element.text();
        if value.is_empty() {
            return None;
        }
        let value = Path::new(value.as_ref());
        let environment = self.environment;
        match element.attribute("prefix") {
            Some("xdg") => {
                let home = match base {
                    Base::Data => environment.data_home.as_ref(),
                    Base::Config => environment.config_home.as_ref(),
                };
                home.map(|home| home.join(value))
            }
            Some("relative") => file.parent().map(|dir| dir.join(value)),
            Some("default" | "cwd") => environment.cwd.as_ref().map(|cwd| cwd.join(value)),
            _ => match environment.expand_home(value) {
                Some(expanded) => Some(expanded),
                None if value.is_absolute() => Some(value.to_owned()),
                None => match base {
                    Base::Data => environment.cwd.as_ref().map(|cwd| cwd.join(value)),
                    Base::Config => environment.find_config(value),
                },
            },
        }
    }

    fn include(&mut self, element: &Element<'_>, file: &Path) {
        let ignore_missing = element.attribute("ignore_missing") == Some("yes");
        let target = self.resolve(element, Base::Config, file);
        match target {
            Some(target) if target.is_dir() => self.include_dir(&target),
            Some(target) if target.is_file() => self.load_file(&target),
            _ if ignore_missing => {}
            target => {
                let missing = target.unwrap_or_else(|| PathBuf::from(element.text().into_owned()));
                self.problems.push(LoadProblem::MissingInclude(missing));
            }
        }
    }

    fn include_dir(&mut self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.as_encoded_bytes();
                name.first().is_some_and(u8::is_ascii_digit)
                    && name.ends_with(CONFIG_EXTENSION.as_bytes())
            })
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        files.sort();
        for file in files {
            self.load_file(&file);
        }
    }

    fn select_fonts(&mut self, element: &Element<'_>) {
        for selection in element.children() {
            let target = match selection.name() {
                "acceptfont" => &mut self.config.accept,
                "rejectfont" => &mut self.config.reject,
                _ => continue,
            };
            target.extend(
                selection
                    .children_named("glob")
                    .map(Element::text)
                    .filter(|pattern| !pattern.is_empty())
                    .map(|pattern| Glob::new(&pattern)),
            );
        }
    }
}

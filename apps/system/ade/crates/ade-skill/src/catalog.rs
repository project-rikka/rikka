use std::fmt;
use std::path::{Path, PathBuf};

use ade_scope::{Diagnostic, Scope, listing};

use crate::skill::{SKILL, Skill};

const SKILLS: &str = "skills";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    skills: Vec<Skill>,
    diagnostics: Vec<Diagnostic>,
}

impl Skills {
    #[must_use]
    pub fn find(project: Option<&Path>) -> Skills {
        let mut catalog = Skills::default();

        for scope in Scope::PARTICULAR_FIRST {
            let Some(folder) = Skills::folder(scope, project) else {
                continue;
            };
            for bundle in listing::folders(&folder) {
                catalog.take(&bundle, scope);
            }
        }

        catalog
            .skills
            .sort_by(|one, two| one.name().cmp(two.name()));
        catalog
    }

    #[must_use]
    pub fn folder(scope: Scope, project: Option<&Path>) -> Option<PathBuf> {
        Some(scope.rikka(project)?.join(SKILLS))
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Skill> {
        let named = |query: &str| self.skills.iter().find(|skill| skill.name() == query);
        named(name).or_else(|| {
            self.skills
                .iter()
                .find(|skill| skill.name().eq_ignore_ascii_case(name))
        })
    }

    #[must_use]
    pub fn all(&self) -> &[Skill] {
        &self.skills
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.skills.iter().map(Skill::name)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn catalogue(&self) -> String {
        let mut catalogue = String::new();
        self.write_catalogue(&mut catalogue)
            .expect("writing to a String cannot fail");
        catalogue
    }

    fn write_catalogue(&self, out: &mut impl fmt::Write) -> fmt::Result {
        out.write_str("<available_skills>")?;
        for skill in &self.skills {
            write!(
                out,
                "\n  <skill>\n    <name>{}</name>\n    <description>{}</description>\n  </skill>",
                skill.name(),
                skill.description()
            )?;
        }
        out.write_str("\n</available_skills>")
    }

    fn take(&mut self, bundle: &Path, scope: Scope) {
        if !bundle.join(SKILL).is_file() {
            let message = format!("no {SKILL} found");
            return self.diagnostics.push(Diagnostic::skipped(bundle, message));
        }

        let (skill, notes) = match Skill::read(bundle, scope) {
            Ok(read) => read,
            Err(reason) => {
                return self
                    .diagnostics
                    .push(Diagnostic::skipped(bundle, reason.to_string()));
            }
        };

        if let Some(had) = self.get(skill.name()) {
            let message = format!(
                "\"{}\" is already a skill, found at {}",
                skill.name(),
                had.location().display()
            );
            return self
                .diagnostics
                .push(Diagnostic::skipped(skill.location(), message));
        }

        for note in notes {
            self.diagnostics
                .push(Diagnostic::warning(skill.location(), note));
        }
        self.skills.push(skill);
    }
}

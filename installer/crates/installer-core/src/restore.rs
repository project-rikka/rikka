use std::collections::BTreeMap;

use rikka_backup::Configured;
use rikka_nix_edit::{Document, Entry, Set};
use rikka_nixos_layout::Module;

use crate::{
    answers::Answers,
    error::{Error, Result},
    keyboard::KEYBOARDS,
    locale::LOCALES,
};

const OVERLAID: [Module; 3] = [Module::Apps, Module::Nix, Module::System];
const ENGLISH_MESSAGES: &str = "en_US.UTF-8";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Start {
    #[default]
    Fresh,
    Backup,
    Repository,
}

impl Start {
    pub const ALL: &'static [Start] = &[Start::Fresh, Start::Backup, Start::Repository];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Fresh => "A fresh install",
            Self::Backup => "A Rikka backup file",
            Self::Repository => "A configuration repository (Git)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    pub modules: Vec<Configured>,
    pub declared_for: Option<String>,
    pub pending: Option<String>,
    pub sign_ins: usize,
    pub preferences: usize,
}

impl Restored {
    pub fn prefill(&self, answers: &mut Answers) -> Result<()> {
        let documents: BTreeMap<Module, Document> = self
            .modules
            .iter()
            .map(|configured| {
                Document::parse(configured.text.clone())
                    .map(|document| (configured.module, document))
            })
            .collect::<rikka_nix_edit::Result<_>>()?;
        if let Some(system) = documents.get(&Module::System) {
            if let Some(hostname) = system.get::<String>("networking.hostName")? {
                answers.hostname = hostname;
            }
            if let Some(zone) = system.get::<String>("time.timeZone")? {
                answers.timezone = zone;
            }
            if let Some(name) = system.get::<String>("i18n.defaultLocale")?
                && let Some(locale) = LOCALES.iter().find(|locale| locale.name == name)
            {
                answers.locale = locale;
            }
            answers.english_messages = system
                .get::<String>("i18n.extraLocaleSettings.LC_MESSAGES")?
                .is_some_and(|messages| messages == ENGLISH_MESSAGES);
            let layout = system.get::<String>("services.xserver.xkb.layout")?;
            let variant = system
                .get::<String>("services.xserver.xkb.variant")?
                .unwrap_or_default();
            if let Some(keyboard) = layout.and_then(|layout| {
                KEYBOARDS
                    .iter()
                    .find(|keyboard| keyboard.layout == layout && keyboard.variant == variant)
            }) {
                answers.keyboard = keyboard;
            }
        }
        if let Some(nix) = documents.get(&Module::Nix)
            && let Some(unfree) = nix.get::<bool>("nixpkgs.config.allowUnfree")?
        {
            answers.allow_unfree = unfree;
        }
        if let Some(apps) = documents.get(&Module::Apps) {
            let ids: Vec<String> = answers
                .catalog
                .optional()
                .map(|component| component.id.clone())
                .collect();
            for id in ids {
                let enabled = apps.get::<bool>(["rikka", "apps", id.as_str(), "enable"])?;
                answers.choose_app(&id, enabled.unwrap_or(false));
            }
        }
        if answers.username.is_empty()
            && let Some(user) = &self.declared_for
        {
            answers.username.clone_from(user);
        }
        Ok(())
    }

    pub fn render(&self, module: Module, generated: &Set, user: &str) -> Result<Option<String>> {
        let Some(configured) = self
            .modules
            .iter()
            .find(|configured| configured.module == module)
        else {
            return Ok(None);
        };
        let mut documents = Configured::documents(
            std::slice::from_ref(configured),
            self.declared_for.as_deref(),
            user,
        )
        .map_err(|error| Error::Restore(error.to_string()))?;
        let Some(mut document) = documents.remove(&module) else {
            return Ok(None);
        };
        if OVERLAID.contains(&module) {
            for entry in generated.entries() {
                if let Entry::Binding { path, value } = entry {
                    document.set(path, value.clone())?;
                }
            }
        }
        Ok(Some(document.to_string()))
    }
}

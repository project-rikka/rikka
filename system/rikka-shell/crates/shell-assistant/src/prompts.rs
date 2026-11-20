use std::path::Path;

use rikka_assist::App;
use rikka_prompt::{Facts, Prompt, Result};
use rikka_session::profile::Profile;
use rikka_thread::ToolKitEntry;

const SYSTEM: &str = include_str!("../prompts/system.md");
const ENVIRONMENT: &str = include_str!("../prompts/environment.md");
const COMPACT: &str = include_str!("../prompts/compact.md");
const APPS: &str = include_str!("../prompts/apps.md");
const POINTED: &str = include_str!("../prompts/pointed.md");

#[derive(Clone, Debug)]
pub(crate) struct Prompts {
    system: Prompt,
    environment: Prompt,
    compact: Prompt,
    apps: Prompt,
    pointed: Prompt,
    tools: Facts,
}

impl Prompts {
    pub(crate) fn load() -> Result<Prompts> {
        Ok(Prompts {
            system: Prompt::read("system", SYSTEM)?,
            environment: Prompt::read("environment", ENVIRONMENT)?,
            compact: Prompt::read("compact", COMPACT)?,
            apps: Prompt::read("apps", APPS)?,
            pointed: Prompt::read("pointed", POINTED)?,
            tools: rikka_tools::facts()?,
        })
    }

    pub(crate) fn facts(&self, home: &Path, language: &str) -> Result<Facts> {
        let mut facts = rikka_tools::facts_for(language)?;
        facts.set("home", home.display().to_string());
        facts.set("desktop", desktop());
        Ok(facts)
    }

    pub(crate) fn system(&self, facts: &Facts) -> Result<String> {
        self.system.body(facts)
    }

    pub(super) fn environment(&self, facts: &Facts) -> Result<String> {
        self.environment.body(facts)
    }

    pub(crate) fn compact(&self, facts: &Facts) -> Result<String> {
        self.compact.body(facts)
    }

    pub(crate) fn apps(&self, running: &[App]) -> Result<String> {
        let mut facts = self.tools.clone();
        let listed = match running.is_empty() {
            true => self.apps.param("none", &facts)?,
            false => running
                .iter()
                .map(|app| format!("- {} ({}): {}", app.kit(), app.title(), app.about()))
                .collect::<Vec<String>>()
                .join("\n"),
        };
        facts.set("apps", listed);
        self.apps.body(&facts)
    }

    pub(crate) fn pointed(&self, app: &App, kit_state: &ToolKitEntry) -> Result<String> {
        let mut facts = self.tools.clone();
        let kit = kit_state.toolkit();
        let tools: Vec<&str> = kit
            .functions()
            .iter()
            .map(|function| function.name())
            .collect();
        facts.set("app", app.kit());
        facts.set("title", app.title());
        facts.set("tools", tools.join(", "));
        facts.set(
            "instructions",
            match kit_state.is_first() {
                true => kit.instructions().trim(),
                false => "",
            },
        );
        Ok(self.pointed.body(&facts)?.trim_end().to_owned())
    }
}

fn desktop() -> &'static str {
    match Profile::from_environment() {
        Profile::Full => "Rikka",
        Profile::Simple => "Rikka, nested in another session",
    }
}

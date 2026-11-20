use rikka_assist::App;
use rikka_thread::ToolKitEntry;

use crate::prompts::Prompts;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ReportedContext {
    apps: Option<Vec<String>>,
}

#[derive(Debug, Default)]
pub(super) struct TurnContext {
    pub(crate) context: Vec<String>,
    pub(crate) opened_kits: Vec<String>,
}

impl ReportedContext {
    pub(super) fn next_turn(
        &mut self,
        prompts: &Prompts,
        message: &str,
        running: &[App],
        mut peek: impl FnMut(&str) -> Option<ToolKitEntry>,
    ) -> TurnContext {
        let mut turn_context = TurnContext::default();

        let mut kits: Vec<String> = running.iter().map(|app| app.kit().to_owned()).collect();
        kits.sort_unstable();
        let changed = match &self.apps {
            Some(reported) => *reported != kits,
            None => !kits.is_empty(),
        };
        if changed {
            match prompts.apps(running) {
                Ok(note) => {
                    turn_context.context.push(note);
                    self.apps = Some(kits);
                }
                Err(error) => tracing::warn!("failed to render running apps prompt: {error}"),
            }
        }

        for app in pointed(message, running) {
            let Some(opened_kits) = peek(app.kit()) else {
                continue;
            };
            match prompts.pointed(app, &opened_kits) {
                Ok(note) => {
                    turn_context.context.push(note);
                    turn_context.opened_kits.push(app.kit().to_owned());
                }
                Err(error) => {
                    tracing::warn!("failed to render mention prompt for {}: {error}", app.kit())
                }
            }
        }

        turn_context
    }
}

pub(super) fn points_at(message: &str, kit: &str) -> bool {
    message
        .split_whitespace()
        .filter_map(|word| word.strip_prefix('@'))
        .any(|named| lead(named).eq_ignore_ascii_case(kit))
}

pub(super) fn names_app(named: &str, running: &[App]) -> bool {
    found(named, running).is_some()
}

fn found<'a>(named: &str, running: &'a [App]) -> Option<&'a App> {
    let lead = lead(named);
    running.iter().find(|app| {
        (!lead.is_empty() && app.kit().eq_ignore_ascii_case(lead))
            || (!app.title().is_empty() && named.starts_with(app.title()))
    })
}

fn pointed<'a>(message: &str, running: &'a [App]) -> Vec<&'a App> {
    let mut pointed: Vec<&App> = Vec::new();
    for named in message
        .split_whitespace()
        .filter_map(|word| word.strip_prefix('@'))
    {
        if let Some(app) = found(named, running)
            && !pointed.contains(&app)
        {
            pointed.push(app);
        }
    }
    pointed
}

fn lead(named: &str) -> &str {
    let end = named
        .find(|letter: char| !letter.is_ascii_alphanumeric())
        .unwrap_or(named.len());
    &named[..end]
}

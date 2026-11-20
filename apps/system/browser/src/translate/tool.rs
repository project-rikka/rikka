use crate::shell::Shell;
use crate::translate::decide;
use crate::translate::tabs::Progress;
use browser_model::TabId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Offered,
    Busy,
    Translated,
    Failed,
}

pub(crate) fn tool(shell: &Shell, tab: TabId, address: &str) -> Option<Tool> {
    let status = shell.browser().translations().tabs().status(tab);
    if status.busy {
        return Some(Tool::Busy);
    }
    match status.progress {
        Progress::Translating | Progress::Translated => return Some(Tool::Translated),
        Progress::Failed(_) => return Some(Tool::Failed),
        Progress::Original => {}
    }
    let preferences = shell.browser().preferences();
    decide::decide(preferences, status.detected, address, |page, rules| {
        decide::offered(page, rules, preferences.translate_offer())
    })
    .then_some(Tool::Offered)
}

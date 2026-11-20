use std::fmt;

use rikka_tool::Reply;

use super::layers::{LayerTree, shown_name};
use super::{Paint, ServeResult, selection_said};
use crate::action;
use crate::tab::{Phase, Tab};

const RECENT: usize = 6;

impl Paint {
    pub(super) fn describe(&self, index: usize) -> ServeResult {
        Ok(Reply::new(Description { paint: self, index }.to_string()))
    }
}

struct Description<'a> {
    paint: &'a Paint,
    index: usize,
}

impl fmt::Display for Description<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let paint = self.paint;
        let handles = paint.handles();
        writeln!(f, "Tabs in this window (* active):")?;
        for (at, (tab, handle)) in paint.tabs.iter().zip(&handles).enumerate() {
            let mark = if at == paint.active { "*" } else { " " };
            writeln!(f, "{mark} {handle}: {}", tab_line(tab))?;
        }
        let tab = &paint.tabs[self.index];
        let Phase::Open(_) = tab.phase() else {
            return Ok(());
        };
        writeln!(
            f,
            "\n{}, layers top first (index \"name\" kind, then any of Background, hidden, opacity, blend):",
            handles[self.index]
        )?;
        write!(f, "{}", LayerTree(tab.layers()))?;
        if let Some(active_layer) = tab.active_layer() {
            writeln!(
                f,
                "Chosen layer (the default for Adjust and Draw): {} \"{}\"",
                active_layer.id.index(),
                shown_name(active_layer)
            )?;
        }
        writeln!(
            f,
            "Selection: {}",
            selection_said(&tab.selection().region())
        )?;
        if let Some(session) = tab.session() {
            writeln!(
                f,
                "The {} dialog is open; changes wait until the person applies or cancels it.",
                session.dialog().title()
            )?;
        }
        let history_view = tab.history_view();
        let last = history_view.actions.len() - 1;
        write!(
            f,
            "History: at entry {} of {last}; latest:",
            history_view.current
        )?;
        for at in last.saturating_sub(RECENT - 1)..=last {
            let current = if at == history_view.current { "*" } else { "" };
            let separator = if at == last { "" } else { ";" };
            write!(
                f,
                " {at}{current} \"{}\"{separator}",
                action::name(&history_view.actions[at])
            )?;
        }
        Ok(())
    }
}

fn tab_line(tab: &Tab) -> String {
    match tab.phase() {
        Phase::Opening => "opening".to_owned(),
        Phase::Failed(reason) => format!("could not be opened: {reason}"),
        Phase::Open(facts) => {
            let profile = facts
                .profile
                .as_ref()
                .and_then(|profile| profile.name())
                .unwrap_or("untagged (sRGB)");
            let modified = match (tab.path(), tab.is_modified()) {
                (None, _) => ", new, never saved",
                (Some(_), true) => ", unsaved changes",
                (Some(_), false) => "",
            };
            format!(
                "{}×{}, {}-bit {}, {profile}, {}{modified}",
                facts.width,
                facts.height,
                facts.bits,
                facts.mode,
                facts.format.name()
            )
        }
    }
}

use rikka_app::prelude::*;
use rikka_web::{ActionKind, ExtensionAction};

use crate::extensions::icons::icon;
use crate::extensions::service::merged;
use crate::shell::Shell;
use browser_model::TabId;

const SIDE: f32 = 16.0;
const BADGE: f32 = 9.0;
const BADGE_AT: f32 = 9.0;
const CLICK: &str = "rikka-browser-extension-click";

pub(crate) fn action_key(id: &str, kind: ActionKind) -> (&'static str, String, bool) {
    (
        "rikka-browser-extension-action",
        id.to_owned(),
        kind == ActionKind::Page,
    )
}

fn color(rgba: [f64; 4]) -> Color {
    let channel = |value: f64| value.clamp(0.0, 255.0) as u8;
    Color::rgba8(
        channel(rgba[0]),
        channel(rgba[1]),
        channel(rgba[2]),
        channel(rgba[3]),
    )
}

fn badge(action: &ExtensionAction) -> Option<Element> {
    let text_shown = action.badge_text.clone().filter(|text| !text.is_empty())?;
    let fill = action
        .badge_background_color
        .map_or(palette().accent, color);
    let ink = action.badge_text_color.map_or(Color::WHITE, color);
    Some(
        container(text(text_shown).text_size(BADGE).color(ink).no_wrap())
            .padding(Inset::xy(Space::HAIR, 0.0))
            .background(fill)
            .radius(BADGE)
            .build()
            .position(BADGE_AT, BADGE_AT),
    )
}

fn button_of(tab: TabId, id: String, kind: ActionKind, action: &ExtensionAction) -> Element {
    let enabled = action.enabled.unwrap_or(true);
    let glyph = column_of(std::iter::once(icon(action.icon.as_ref(), SIDE)).chain(badge(action)));
    let key = action_key(&id, kind);
    button(glyph)
        .ghost()
        .icon_only()
        .small()
        .focusless()
        .enabled(enabled)
        .on_press(move |shell: &mut Shell| shell.click_action(tab, id.clone(), kind))
        .build()
        .keyed(key)
}

pub(crate) fn actions(shell: &Shell, tab: TabId) -> Option<Element> {
    let key = shell.model().tab(tab)?.profile;
    let extensions = shell.browser().extensions();
    let state = extensions.state();
    let enabled: Vec<&str> = state
        .installed
        .get(&key)?
        .iter()
        .filter(|extension| extension.meta_data.enabled)
        .map(|extension| extension.web_extension_id.as_str())
        .collect();
    let mut shown: Vec<(String, ActionKind, ExtensionAction)> = state
        .defaults
        .iter()
        .filter(|((profile, (id, _)), _)| *profile == key && enabled.contains(&id.as_str()))
        .map(|((_, (id, kind)), base)| {
            let over = state.tabs.get(&(tab, (id.clone(), *kind)));
            (id.clone(), *kind, merged(base, over))
        })
        .filter(|(_, kind, action)| *kind == ActionKind::Browser || action.enabled == Some(true))
        .collect();
    drop(state);
    if shown.is_empty() {
        return None;
    }
    shown.sort_by(|left, right| left.0.cmp(&right.0));
    Some(
        row_of(
            shown
                .into_iter()
                .map(|(id, kind, action)| button_of(tab, id, kind, &action)),
        )
        .gap(Space::HAIR)
        .align_items(Align::Center)
        .padding(Inset::edges(0.0, 0.0, 0.0, Space::TIGHT))
        .shrink(0.0)
        .any(),
    )
}

impl Shell {
    pub(crate) fn click_action(&mut self, tab: TabId, id: String, kind: ActionKind) {
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            return;
        };
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        let extension_id = id.clone();
        spawn_keyed(
            (CLICK, id.clone()),
            async move {
                let extensions = runtime.extensions();
                match kind {
                    ActionKind::Browser => extensions.click_browser_action(extension_id).await,
                    ActionKind::Page => extensions.click_page_action(extension_id).await,
                }
            },
            move |shell: &mut Shell, clicked: rikka_web::Result<Option<String>>| match clicked {
                Ok(Some(uri)) => shell.open_action_popup(tab, id.clone(), kind, uri),
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "extension action failed"),
            },
        );
    }
}

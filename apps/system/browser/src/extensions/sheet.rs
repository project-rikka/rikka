use rikka_app::prelude::*;
use rikka_web::{Extension, PermissionSet};
use rikka_widgets::{column, row};

use crate::extensions::icons::icon;
use crate::extensions::service::{Ask, Asking};
use crate::requests::{framed_card, scrimmed};
use crate::shell::Shell;
use crate::widgets::confirm_keys;
use browser_model::WindowId;

const ICON: f32 = 32.0;
const LISTED: usize = 12;

impl Shell {
    pub(crate) fn answer_extension(&mut self, allowed: bool) {
        let Some(Asking { ask, .. }) = self.browser().extensions().take_asking(self.id()) else {
            return;
        };
        match (ask, allowed) {
            (Ask::Install(request), true) => request.allow(false, false),
            (Ask::Install(request), false) => request.deny(),
            (Ask::Update(request), allowed) => request.allow(allowed),
            (Ask::Permissions(request), allowed) => request.allow(allowed),
        }
    }
}

fn name(extension: &Extension) -> String {
    extension
        .meta_data
        .name
        .clone()
        .unwrap_or_else(|| extension.web_extension_id.clone())
}

fn permission_names(permissions: &PermissionSet) -> Vec<String> {
    permissions
        .permissions
        .iter()
        .chain(&permissions.origins)
        .chain(&permissions.data_collection)
        .cloned()
        .collect()
}

pub(crate) fn sheet(shell: &Shell, window: WindowId) -> Option<Element> {
    let extensions = shell.browser().extensions();
    let state = extensions.state();
    let asking = state.asking.iter().find(|asking| asking.window == window)?;
    let installed = match &asking.ask {
        Ask::Permissions(request) => state.installed.get(&asking.key).and_then(|listed| {
            listed
                .iter()
                .find(|extension| extension.web_extension_id == request.extension_id)
        }),
        Ask::Install(_) | Ask::Update(_) => None,
    };
    let (title, extension, permissions, accept) = match &asking.ask {
        Ask::Install(request) => (
            tr!(
                "browser.extensions.install-title",
                name = &name(&request.extension)
            ),
            Some(&request.extension),
            permission_names(&request.permissions),
            tr!("browser.extensions.add"),
        ),
        Ask::Update(request) => (
            tr!(
                "browser.extensions.update-title",
                name = &name(&request.extension)
            ),
            Some(&request.extension),
            permission_names(&request.added),
            tr!("browser.extensions.allow"),
        ),
        Ask::Permissions(request) => (
            tr!(
                "browser.extensions.permissions-title",
                name = &installed.map_or_else(|| request.extension_id.clone(), name)
            ),
            installed,
            permission_names(&request.permissions),
            tr!("browser.extensions.allow"),
        ),
    };
    let symbol = icon(
        extension.and_then(|extension| extension.meta_data.icons.as_ref()),
        ICON,
    );
    let more = permissions.len().saturating_sub(LISTED);
    let listed = column_of(
        permissions
            .iter()
            .take(LISTED)
            .map(|permission| text(format!("• {permission}")).caption().any()),
    )
    .gap(Space::HAIR);
    let card = framed_card(
        column! {
            row! {
                symbol;
                text(title).label().semibold().grow(1.0).min_width(0.0);
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width();
            (!permissions.is_empty())
                .then(|| text(tr!("browser.extensions.wants")).caption().muted());
            listed;
            (more > 0).then(|| {
                text(tr!("browser.extensions.more", count = more as i64))
                    .caption()
                    .muted()
            });
            row! {
                spacer().grow(1.0);
                button(text(tr!("browser.dialog.cancel")))
                    .secondary()
                    .on_press(|shell: &mut Shell| shell.answer_extension(false));
                button(text(accept))
                    .primary()
                    .on_press(|shell: &mut Shell| shell.answer_extension(true));
            }
            .gap(Space::SNUG)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width(),
    );
    drop(state);
    Some(scrimmed(confirm_keys(
        card,
        |shell: &mut Shell| shell.answer_extension(true),
        |shell: &mut Shell| shell.answer_extension(false),
    )))
}

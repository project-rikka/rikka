use rikka_app::prelude::*;

use crate::services::Services;
use crate::tab::Tab;
use crate::window::Dialog;
use crate::{listing, wording};

pub(super) fn dialog(services: &Services, tab: &Tab) -> Option<Dialog> {
    let chosen = tab.selected_entries();
    if chosen.is_empty() {
        return None;
    }
    if chosen.len() > 1 {
        let bytes: u64 = chosen.iter().map(|entry| entry.size).sum();
        let folders = chosen.iter().filter(|entry| entry.is_directory()).count();
        return Some(Dialog::Properties {
            heading: tr!("files.properties.items", count = chosen.len()).to_string(),
            lines: vec![
                (
                    tr!("files.properties.folders").to_string(),
                    folders.to_string(),
                ),
                (
                    tr!("files.properties.files").to_string(),
                    (chosen.len() - folders).to_string(),
                ),
                (
                    tr!("files.properties.total-size").to_string(),
                    wording::size_detailed(bytes),
                ),
            ],
        });
    }

    let entry = chosen[0];
    services
        .mime
        .set_language(&rikka_i18n::current_locale().to_string());
    let mut lines = vec![
        (
            tr!("files.properties.kind").to_string(),
            tr!(
                "files.properties.kind-detail",
                description = &listing::describe(services, entry),
                mime = &entry.mime.to_string(),
            )
            .to_string(),
        ),
        (
            tr!("files.properties.location").to_string(),
            entry
                .location
                .parent()
                .map_or_else(String::new, |at| at.display()),
        ),
    ];
    if !entry.is_directory() {
        lines.push((
            tr!("files.properties.size").to_string(),
            wording::size_detailed(entry.size),
        ));
    }
    lines.push((
        tr!("files.properties.modified").to_string(),
        files_core::format::timestamp(entry.modified),
    ));
    if entry.created.is_some() {
        lines.push((
            tr!("files.properties.created").to_string(),
            files_core::format::timestamp(entry.created),
        ));
    }
    lines.push((
        tr!("files.properties.permissions").to_string(),
        files_core::format::permissions(entry.mode),
    ));
    if let Some(link) = &entry.link {
        lines.push((
            tr!("files.properties.target").to_string(),
            link.target.to_string_lossy().into_owned(),
        ));
    }
    Some(Dialog::Properties {
        heading: entry.name.clone(),
        lines,
    })
}

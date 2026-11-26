use std::collections::BTreeSet;

use rikka_app::prelude::*;
use rikka_backup::Bundle;
use rikka_secrets::schema;
use rikka_widgets::{column, row};
use settings_backup::Published;

use crate::backup::component::{Backup, Report, Work};
use crate::keyring::app_named;
use crate::notice::notice;

const FIELD: f32 = 300.0;

pub(crate) fn view(backup: &Backup) -> impl View {
    column! {
        backup.report.as_ref().map(report);
        if let Err(reason) = &backup.account {
            notice(MaterialSymbol::ERROR, palette().danger, tr!("settings.backup.unavailable", reason = reason.as_str()));
        } else {
            export(backup);
            restore(backup);
            repository(backup);
        }
    }
    .gap(Space::ROOMY)
}

fn report(report: &Report) -> Container {
    match report {
        Report::Exported { path, moved } => notice(
            MaterialSymbol::CHECK_CIRCLE,
            palette().success,
            tr!(
                "settings.backup.exported",
                path = path.display().to_string().as_str(),
                moved = *moved
            ),
        ),
        Report::Restored { staged, skipped } => notice(
            MaterialSymbol::CHECK_CIRCLE,
            palette().success,
            match skipped.first() {
                None => tr!("settings.backup.restored", staged = *staged),
                Some(first) => tr!(
                    "settings.backup.restored-skipping",
                    staged = *staged,
                    count = skipped.len(),
                    first = first.as_str()
                ),
            },
        ),
        Report::Published(Published::Pushed) => notice(
            MaterialSymbol::CLOUD_DONE,
            palette().success,
            tr!("settings.backup.published"),
        ),
        Report::Published(Published::Unchanged) => notice(
            MaterialSymbol::CLOUD_DONE,
            palette().success,
            tr!("settings.backup.up-to-date"),
        ),
        Report::Fetched { staged } => notice(
            MaterialSymbol::CLOUD_DOWNLOAD,
            palette().success,
            tr!("settings.backup.fetched", staged = *staged),
        ),
        Report::Failed(reason) => notice(
            MaterialSymbol::ERROR,
            palette().danger,
            tr!("settings.backup.failed", reason = reason.as_str()),
        ),
    }
}

fn export(backup: &Backup) -> impl View {
    let typed = !backup.confirm.is_empty();
    let mismatch = typed && !backup.passphrase_ready();
    section(
        tr!("settings.backup.export"),
        column! {
            text(tr!("settings.backup.export-contents")).caption().muted();
            field(
                tr!("settings.backup.sign-ins"),
                switch(backup.sign_ins).on_toggle(|backup: &mut Backup, on: bool| {
                    backup.include_sign_ins(on);
                }),
            );
            if backup.sign_ins {
                text(tr!("settings.backup.sign-ins-move")).caption().color(palette().warning);
            }
            field(
                tr!("settings.backup.passphrase"),
                secret(backup, &backup.passphrase),
            );
            field(
                tr!("settings.backup.confirm"),
                secret(backup, &backup.confirm)
                    .error(mismatch)
                    .on_submit(Backup::save),
            );
            if mismatch {
                text(tr!("settings.backup.mismatch")).caption().color(palette().danger);
            }
            row! {
                button(icon(match backup.revealed {
                    true => MaterialSymbol::VISIBILITY_OFF,
                    false => MaterialSymbol::VISIBILITY,
                }))
                .ghost()
                .icon_only()
                .on_press(Backup::reveal)
                .tooltip(match backup.revealed {
                    true => tr!("settings.backup.hide"),
                    false => tr!("settings.backup.show"),
                });
                spacer().grow(1.0);
                button(tr!("settings.backup.save"))
                    .primary()
                    .loading(backup.work == Some(Work::Exporting))
                    .enabled(!backup.is_busy() && backup.passphrase_ready())
                    .on_press(Backup::save);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn restore(backup: &Backup) -> impl View {
    let body: Element = match (&backup.opened, &backup.chosen) {
        (Some(opened), _) => contents(backup, &opened.bundle).build(),
        (None, Some(path)) => column! {
            if backup.account.as_ref().is_ok_and(|account| account.dirs.pending() == *path) {
                notice(MaterialSymbol::INSTALL_DESKTOP, palette().accent, tr!("settings.backup.waiting"));
            } else {
                text(Label::from(path.display().to_string())).label();
            }
            field(
                tr!("settings.backup.passphrase"),
                secret(backup, &backup.unlock).autofocus(true).on_submit(Backup::open),
            );
            row! {
                button(tr!("settings.backup.other"))
                    .secondary()
                    .enabled(!backup.is_busy())
                    .on_press(Backup::choose);
                spacer().grow(1.0);
                button(tr!("settings.backup.unlock"))
                    .primary()
                    .loading(backup.work == Some(Work::Opening))
                    .enabled(!backup.is_busy() && !backup.unlock.is_empty())
                    .on_press(Backup::open);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width()
        .build(),
        (None, None) => row! {
            text(tr!("settings.backup.restore-hint")).caption().muted().grow(1.0);
            button(tr!("settings.backup.choose"))
                .secondary()
                .enabled(!backup.is_busy())
                .on_press(Backup::choose);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .build(),
    };
    section(tr!("settings.backup.restore"), body)
}

fn contents(backup: &Backup, bundle: &Bundle) -> impl View {
    let apps: BTreeSet<String> = bundle
        .credentials
        .iter()
        .map(|credential| {
            app_named(
                credential
                    .attributes
                    .get(schema::ATTRIBUTE)
                    .map(String::as_str),
            )
            .to_string()
        })
        .collect();
    let apps = apps.into_iter().collect::<Vec<_>>().join(", ");
    let created = jiff::Timestamp::from_second(bundle.origin.created as i64)
        .map(|stamp| stamp.strftime("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();
    column! {
        text(tr!(
            "settings.backup.from",
            host = bundle.origin.host.as_str(),
            user = bundle.origin.user.as_str(),
            created = created.as_str()
        ))
        .label();
        text(tr!(
            "settings.backup.holds",
            modules = bundle.modules.len(),
            preferences = bundle.preferences.len()
        ))
        .caption()
        .muted();
        if !apps.is_empty() {
            text(tr!("settings.backup.holds-sign-ins", apps = apps.as_str())).caption().muted();
        }
        text(tr!("settings.backup.replaces")).caption().color(palette().warning);
        row! {
            button(tr!("settings.backup.cancel"))
                .secondary()
                .enabled(!backup.is_busy())
                .on_press(Backup::forget_opened);
            spacer().grow(1.0);
            button(tr!("settings.backup.restore-now"))
                .primary()
                .loading(backup.work == Some(Work::Restoring))
                .enabled(!backup.is_busy())
                .on_press(Backup::restore);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn repository(backup: &Backup) -> impl View {
    let typed = !backup.repository.is_empty();
    section(
        tr!("settings.backup.repository"),
        column! {
            text(tr!("settings.backup.repository-contents")).caption().muted();
            field(
                tr!("settings.backup.repository-url"),
                text_field(&backup.repository)
                    .placeholder(tr!("settings.backup.repository-placeholder"))
                    .on_change(|| {})
                    .on_blur(Backup::remember_repository)
                    .on_submit(Backup::remember_repository)
                    .width(FIELD),
            );
            row! {
                spacer().grow(1.0);
                button(tr!("settings.backup.fetch"))
                    .secondary()
                    .loading(backup.work == Some(Work::Fetching))
                    .enabled(!backup.is_busy() && typed)
                    .on_press(Backup::fetch);
                button(tr!("settings.backup.publish"))
                    .primary()
                    .loading(backup.work == Some(Work::Publishing))
                    .enabled(!backup.is_busy() && typed)
                    .on_press(Backup::publish);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn secret(backup: &Backup, document: &TextDocument) -> TextField {
    text_field(document)
        .secret(!backup.revealed)
        .enabled(!backup.is_busy())
        .on_change(|| {})
        .width(FIELD)
}

use rikka_app::prelude::*;
use rikka_nix::{Document, Module, Outcome};
use rikka_seam::SeamClient;
use rikka_settings::seam::CAPABILITY;
use rikka_settings::{Scope, Source};
use rikka_widgets::{column, row};
use settings_declared::{Setting, SettingId, Survey, declares, expr, path};

use crate::configuration::values::{self, Held, Values, Write};
use crate::notice::notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

const FOLLOWING: &str = "configuration-following";
const READING: &str = "configuration-reading";

#[derive(Debug, Default)]
pub(crate) struct Configuration {
    account: Option<String>,
    seam: Option<SeamClient>,
    survey: Option<Survey>,
    failure: Option<String>,
    held: Vec<Held>,
}

impl Page for Configuration {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.account = rikka_dirs::passwd::current().map(|account| account.name);
        self.seam = session.seam(CAPABILITY).ok();
        if let Some(seam) = &self.seam {
            stream_keyed(
                FOLLOWING,
                seam.subscribe_to(CAPABILITY),
                |configuration: &mut Configuration, _| configuration.read(),
            );
        }
        self.read();
    }

    fn closed(&mut self) {
        cancel(FOLLOWING);
        cancel(READING);
    }
}

impl Configuration {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn read(&mut self) {
        spawn_blocking_keyed(
            READING,
            values::survey,
            |configuration: &mut Configuration, read: Result<Survey, String>| match read {
                Ok(survey) => {
                    configuration.survey = Some(survey);
                    configuration.failure = None;
                }
                Err(reason) => configuration.failure = Some(reason),
            },
        );
    }

    fn revert(&mut self, id: SettingId) {
        let seam = self.seam.clone();
        spawn_blocking(
            move || Values::connect(seam.as_ref())?.write(Write { id, value: None }),
            |configuration: &mut Configuration, reset: Result<(), String>| {
                configuration.failure = reset.err();
                configuration.read();
            },
        );
    }

    pub(crate) fn declare(&mut self, pending: &mut Pending, chosen: &[SettingId]) {
        let (Some(account), Some(survey)) = (&self.account, &self.survey) else {
            return;
        };
        for setting in chosen.iter().filter_map(|id| survey.find(id)) {
            if !setting.is_changed() {
                continue;
            }
            let Some(value) = &setting.user else {
                continue;
            };
            match path(account, setting) {
                Ok(path) => pending.set(Module::Settings, &path, expr(value)),
                Err(error) => {
                    self.failure = Some(error.to_string());
                    return;
                }
            }
            self.held.retain(|held| held.id != setting.id);
            self.held.push(Held {
                id: setting.id.clone(),
                value: value.clone(),
            });
        }
    }

    pub(crate) fn undeclare(&mut self, pending: &mut Pending, id: &SettingId) {
        let (Some(account), Some(survey)) = (&self.account, &self.survey) else {
            return;
        };
        let Some(setting) = survey.find(id) else {
            return;
        };
        match path(account, setting) {
            Ok(path) => pending.remove(Module::Settings, &path),
            Err(error) => self.failure = Some(error.to_string()),
        }
    }

    pub(crate) fn rebuild_finished(&mut self, pending: &Pending) {
        if !matches!(pending.concluded(), Some(Outcome::Done { .. })) {
            return;
        }
        let held = std::mem::take(&mut self.held);
        let seam = self.seam.clone();
        spawn_blocking(
            move || Values::connect(seam.as_ref())?.settle(&held),
            |configuration: &mut Configuration, settled: Result<(), String>| {
                configuration.failure = settled.err();
                configuration.read();
            },
        );
    }
}

pub(crate) fn view(configuration: &Configuration, pending: &Pending) -> impl View {
    column! {
        configuration.failure.as_deref().map(|reason| {
            notice(
                MaterialSymbol::ERROR,
                palette().warning,
                tr!("settings.configuration.failed", reason = reason),
            )
        });
        if let (Some(survey), Some(account)) = (&configuration.survey, &configuration.account) {
            problems(survey);
            changes(survey, account, pending);
            declarations(survey, account, pending);
        }
    }
    .gap(Space::ROOMY)
}

fn problems(survey: &Survey) -> Option<Container> {
    let first = survey.problems().first()?;
    let place = match &first.key {
        Some(key) => format!("{}/{key}", first.domain),
        None => first.domain.clone(),
    };
    Some(notice(
        MaterialSymbol::WARNING,
        palette().warning,
        tr!(
            "settings.configuration.invalid",
            count = survey.problems().len(),
            place = place.as_str(),
            reason = first.reason.as_str()
        ),
    ))
}

fn changes(survey: &Survey, account: &str, pending: &Pending) -> impl View {
    let changed: Vec<&Setting> = survey.changed().collect();
    let edited = pending.edited(Module::Settings);
    let unstaged: Vec<SettingId> = changed
        .iter()
        .filter(|setting| !staged(edited, account, setting))
        .map(|setting| setting.id.clone())
        .collect();
    let body: Element = match changed.is_empty() {
        true => text(tr!("settings.configuration.no-changes"))
            .caption()
            .muted()
            .build(),
        false => column! {
            column_of(changed.iter().map(|setting| {
                change(setting, staged(edited, account, setting), pending.busy())
            }))
            .gap(Space::SNUG);
            if !unstaged.is_empty() {
                button(tr!("settings.configuration.save-all"))
                    .primary()
                    .enabled(!pending.busy())
                    .on_press(move |settings: &mut crate::settings::Settings| {
                        settings.declare_settings(&unstaged);
                    })
                    .align_self(Align::Start);
            }
        }
        .gap(Space::NORMAL)
        .build(),
    };
    section(tr!("settings.configuration.changes"), body)
}

fn change(setting: &Setting, staged: bool, busy: bool) -> impl View {
    let chosen = vec![setting.id.clone()];
    let id = setting.id.clone();
    let now = setting
        .user
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let was = setting.beneath.to_string();
    let from = origin(setting.beneath_source).to_string();
    row! {
        column! {
            text(Label::from(identifier(setting))).label();
            text(tr!(
                "settings.configuration.changed",
                now = now.as_str(),
                was = was.as_str(),
                origin = from.as_str()
            ))
            .caption()
            .muted()
        }
        .gap(Space::TIGHT)
        .grow(1.0);
        if staged {
            text(tr!("settings.configuration.staged")).caption().muted();
        } else {
            button(tr!("settings.configuration.save"))
                .small()
                .enabled(!busy)
                .on_press(move |settings: &mut crate::settings::Settings| {
                    settings.declare_settings(&chosen);
                });
        }
        button(icon(MaterialSymbol::UNDO))
            .ghost()
            .small()
            .icon_only()
            .on_press(move |configuration: &mut Configuration| {
                configuration.revert(id.clone());
            })
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn declarations(survey: &Survey, account: &str, pending: &Pending) -> impl View {
    let declared: Vec<&Setting> = survey.declared().collect();
    let body: Element = match declared.is_empty() {
        true => text(tr!("settings.configuration.nothing-declared"))
            .caption()
            .muted()
            .build(),
        false => column_of(
            declared
                .iter()
                .map(|setting| declaration(setting, account, pending)),
        )
        .gap(Space::SNUG)
        .build(),
    };
    section(tr!("settings.configuration.declared"), body)
}

fn declaration(setting: &Setting, account: &str, pending: &Pending) -> impl View {
    let (source, value) = match setting.source.is_declared() {
        true => (setting.source, setting.value.to_string()),
        false => (setting.beneath_source, setting.beneath.to_string()),
    };
    let from = origin(source).to_string();
    let saved = pending
        .saved(Module::Settings)
        .and_then(|document| declares(document, account, setting))
        .is_some();
    let removing = saved
        && pending
            .edited(Module::Settings)
            .and_then(|document| declares(document, account, setting))
            .is_none();
    let id = setting.id.clone();
    row! {
        if source.is_locked() {
            icon(MaterialSymbol::LOCK).color(palette().text_muted);
        }
        column! {
            text(Label::from(identifier(setting))).label();
            text(tr!(
                "settings.configuration.declared-as",
                value = value.as_str(),
                origin = from.as_str()
            ))
            .caption()
            .muted()
        }
        .gap(Space::TIGHT)
        .grow(1.0);
        if removing {
            text(tr!("settings.configuration.removing")).caption().muted();
        } else if saved {
            button(tr!("settings.configuration.remove"))
                .small()
                .ghost()
                .enabled(!pending.busy())
                .on_press(move |settings: &mut crate::settings::Settings| {
                    settings.undeclare_setting(&id);
                });
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn staged(edited: Option<&Document>, account: &str, setting: &Setting) -> bool {
    let (Some(document), Some(value)) = (edited, &setting.user) else {
        return false;
    };
    declares(document, account, setting).is_some_and(|declared| declared == expr(value))
}

fn identifier(setting: &Setting) -> String {
    match &setting.instance {
        Some(instance) => format!("{} › {instance} › {}", setting.id.domain, setting.key),
        None => format!("{} › {}", setting.id.domain, setting.key),
    }
}

fn origin(source: Source) -> Label {
    match source {
        Source::Default => tr!("settings.configuration.origin.default"),
        Source::System => tr!("settings.configuration.origin.system"),
        Source::Declared(Scope::Machine) => tr!("settings.configuration.origin.machine"),
        Source::Declared(Scope::User) => tr!("settings.configuration.origin.user"),
        Source::User => tr!("settings.configuration.origin.changed"),
        Source::Locked(_) => tr!("settings.configuration.origin.locked"),
    }
}

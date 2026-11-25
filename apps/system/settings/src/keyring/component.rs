use rikka_app::prelude::*;
use rikka_secrets::{Change, ItemId, KeyringState, Secrets, Status, StoredItem, schema};
use rikka_widgets::{column, row};

use crate::following::following;
use crate::notice::notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

const FOLLOWING: &str = "keyring-following";
const READING: &str = "keyring-reading";

const APPS: [(&str, &str); 7] = [
    (schema::LLM, "settings.keyring.app.llm"),
    (schema::MCP, "settings.keyring.app.mcp"),
    (schema::DISCORD, "settings.keyring.app.discord"),
    (schema::MUSIC, "settings.keyring.app.music"),
    (schema::PROTON, "settings.keyring.app.proton"),
    (schema::REMOTE, "settings.keyring.app.remote"),
    (schema::MINECRAFT, "settings.keyring.app.minecraft"),
];

#[derive(Debug, Default)]
pub(crate) struct Keyring {
    status: Option<Status>,
    items: Vec<StoredItem>,
    failure: Option<String>,
}

struct Read {
    status: Status,
    items: Vec<StoredItem>,
}

impl Page for Keyring {
    fn opened(&mut self, _: &Pending, _: &Session) {
        self.read();
        stream_keyed(
            FOLLOWING,
            following(async { Secrets::shared().await?.changes().await }),
            |keyring: &mut Keyring, change: Result<Change, String>| match change {
                Ok(_) => keyring.read(),
                Err(reason) => keyring.failure = Some(reason),
            },
        );
    }

    fn closed(&mut self) {
        cancel(FOLLOWING);
    }
}

impl Keyring {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn read(&mut self) {
        spawn_keyed(
            READING,
            async move {
                let secrets = Secrets::shared().await?;
                let status = secrets.keyring().status().await?;
                let mut items = secrets.items().await?;
                items.sort_by_key(|item| std::cmp::Reverse(item.modified));
                Ok(Read { status, items })
            },
            |keyring: &mut Keyring, read: rikka_secrets::Result<Read>| match read {
                Ok(read) => {
                    keyring.status = Some(read.status);
                    keyring.items = read.items;
                    keyring.failure = None;
                }
                Err(error) => keyring.failure = Some(error.to_string()),
            },
        );
    }

    fn unlock(&mut self) {
        spawn(
            async move { Secrets::shared().await?.unlock().await },
            |keyring: &mut Keyring, unlocked: rikka_secrets::Result<()>| match unlocked {
                Ok(()) | Err(rikka_secrets::Error::Dismissed) => keyring.read(),
                Err(error) => keyring.failure = Some(error.to_string()),
            },
        );
    }

    fn delete(&mut self, item: ItemId) {
        spawn(
            async move { Secrets::shared().await?.delete(&item).await },
            |keyring: &mut Keyring, deleted: rikka_secrets::Result<()>| match deleted {
                Ok(()) => keyring.read(),
                Err(error) => keyring.failure = Some(error.to_string()),
            },
        );
    }
}

pub(crate) fn view(keyring: &Keyring) -> impl View {
    column! {
        keyring.failure.as_deref().map(|reason| {
            notice(
                MaterialSymbol::CLOUD_OFF,
                palette().warning,
                tr!("settings.keyring.unavailable", reason = reason),
            )
        });
        if let Some(status) = &keyring.status {
            section(tr!("settings.keyring.keyring"), state(status));
            section(tr!("settings.keyring.items"), items(status, &keyring.items));
        }
    }
    .gap(Space::ROOMY)
}

fn state(status: &Status) -> impl View {
    let locked = matches!(
        status.state,
        KeyringState::Locked | KeyringState::Refused | KeyringState::Absent
    );
    column! {
        field(
            tr!("settings.keyring.state"),
            text(state_named(status.state)).label().align(TextAlign::End),
        );
        field(
            tr!("settings.keyring.auto-unlock"),
            text(match status.auto_unlock {
                true => tr!("settings.keyring.auto-unlock-on"),
                false => tr!("settings.keyring.auto-unlock-off"),
            })
            .label()
            .align(TextAlign::End),
        );
        text(tr!("settings.keyring.auto-unlock-hint")).caption().muted();
        if locked {
            button(tr!("settings.keyring.unlock"))
                .primary()
                .on_press(Keyring::unlock)
                .align_self(Align::Start);
        }
    }
    .gap(Space::SNUG)
}

fn items(status: &Status, items: &[StoredItem]) -> Element {
    if status.state != KeyringState::Unlocked {
        return text(tr!("settings.keyring.locked-items"))
            .caption()
            .muted()
            .build();
    }
    if items.is_empty() {
        return text(tr!("settings.keyring.empty"))
            .caption()
            .muted()
            .build();
    }
    column_of(items.iter().map(item)).gap(Space::SNUG).build()
}

fn item(stored: &StoredItem) -> impl View {
    let id = stored.id.clone();
    let modified = i64::try_from(stored.modified)
        .ok()
        .filter(|seconds| *seconds > 0)
        .map(rikka_format::format_timestamp)
        .unwrap_or_default();
    let app = app_named(stored.schema()).to_string();
    row! {
        column! {
            text(Label::from(stored.label.as_str())).label();
            text(tr!(
                "settings.keyring.owner",
                app = app.as_str(),
                date = modified.as_str()
            ))
            .caption()
            .muted()
        }
        .gap(Space::TIGHT)
        .grow(1.0);
        button(icon(MaterialSymbol::DELETE))
            .ghost()
            .small()
            .icon_only()
            .on_press(move |keyring: &mut Keyring| keyring.delete(id.clone()))
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn state_named(state: KeyringState) -> Label {
    match state {
        KeyringState::Absent => tr!("settings.keyring.state-absent"),
        KeyringState::Locked => tr!("settings.keyring.state-locked"),
        KeyringState::Refused => tr!("settings.keyring.state-refused"),
        KeyringState::Unlocked => tr!("settings.keyring.state-unlocked"),
    }
}

fn app_named(owner: Option<&str>) -> Label {
    match APPS.iter().find(|(known, _)| Some(*known) == owner) {
        Some((_, key)) => Label::translated(key),
        None => tr!("settings.keyring.app.other"),
    }
}

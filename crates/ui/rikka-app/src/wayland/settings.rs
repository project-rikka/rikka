use rikka_appearance::{Appearance, ResolvedAppearance};
use rikka_seam::net::SocketClient;
use rikka_seam::{Delivery, SeamClient};
use rikka_settings::seam::Settings;
use rikka_settings::seam::protocol;
use rikka_settings::session::{appearance, language};
use rikka_settings::{Schema, Store};

use rikka_theme::Theme;

pub(crate) enum SessionChange {
    Appearance {
        appearance: Box<Appearance>,
        theme: Option<Box<Theme>>,
    },
    Language,
}

pub(crate) fn apply_session_locale() {
    match Store::open(language::schema()) {
        Ok(store) => {
            apply_locale(&store);
        }
        Err(error) => tracing::debug!("failed to read session locale: {error}"),
    }
}

pub(crate) fn follow(
    deliver: impl Fn(SessionChange) -> bool + Send + 'static,
    appearance: Option<Appearance>,
    language: bool,
) {
    if appearance.is_none() && !language {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("rikka-app-session".to_owned())
        .spawn(move || watch(&deliver, appearance, language));
    if let Err(error) = spawned {
        tracing::warn!("failed to watch session settings: {error}");
    }
}

fn watch(deliver: &impl Fn(SessionChange) -> bool, appearance: Option<Appearance>, language: bool) {
    let connection = match SocketClient::from_env() {
        Ok(connection) => connection,
        Err(reason) => {
            tracing::debug!("not watching session settings: {reason}");
            return;
        }
    };
    let mut events = connection.client().poll_events(Some(protocol::CAPABILITY));
    let mut theme_watch =
        appearance.and_then(|appearance| ThemeWatch::open(appearance, connection.client()));
    let mut locale_watch = language
        .then(|| LocaleWatch::open(connection.client()))
        .flatten();

    if let Some(change) = theme_watch.as_mut().and_then(ThemeWatch::current)
        && !deliver(change)
    {
        return;
    }
    if locale_watch
        .as_mut()
        .is_some_and(LocaleWatch::apply_current)
        && !deliver(SessionChange::Language)
    {
        return;
    }
    while let Some(notice) = events.blocking_next() {
        let theme = theme_watch.as_mut().and_then(|watch| watch.handle(&notice));
        let language = locale_watch
            .as_mut()
            .is_some_and(|watch| watch.handle(&notice));
        if let Some(change) = theme
            && !deliver(change)
        {
            return;
        }
        if language && !deliver(SessionChange::Language) {
            return;
        }
    }
}

struct ThemeWatch {
    settings: Settings,
    resolved: ResolvedAppearance,
}

impl ThemeWatch {
    fn open(appearance: Appearance, seam: SeamClient) -> Option<ThemeWatch> {
        Some(ThemeWatch {
            settings: reader(appearance::schema(), seam, "theme")?,
            resolved: ResolvedAppearance::new(appearance),
        })
    }

    fn current(&mut self) -> Option<SessionChange> {
        let before = self.resolved.appearance().clone();
        let theme = self.resolved.refresh(&self.settings);
        self.changed(theme, &before)
    }

    fn handle(&mut self, notice: &Delivery) -> Option<SessionChange> {
        let before = self.resolved.appearance().clone();
        let theme = self.resolved.apply_delivery(&mut self.settings, notice);
        self.changed(theme, &before)
    }

    fn changed(&self, theme: Option<Theme>, before: &Appearance) -> Option<SessionChange> {
        let appearance = self.resolved.appearance();
        if theme.is_none() && appearance == before {
            return None;
        }
        Some(SessionChange::Appearance {
            appearance: Box::new(appearance.clone()),
            theme: theme.map(Box::new),
        })
    }
}

struct LocaleWatch(Settings);

impl LocaleWatch {
    fn open(seam: SeamClient) -> Option<LocaleWatch> {
        Some(LocaleWatch(reader(language::schema(), seam, "language")?))
    }

    fn apply_current(&mut self) -> bool {
        apply_locale(self.0.store())
    }

    fn handle(&mut self, notice: &Delivery) -> bool {
        match notice {
            Delivery::Event(event) => {
                let Some(changed) = protocol::changed(event, language::DOMAIN) else {
                    return false;
                };
                if !changed.changes_key(language::LANGUAGE) {
                    return false;
                }
                self.0.apply_change(&changed);
            }
            Delivery::Lagged(_) => {
                if self.0.refresh().is_err() {
                    return false;
                }
            }
        }
        self.apply_current()
    }
}

fn reader(schema: Schema, seam: SeamClient, what: &str) -> Option<Settings> {
    Settings::mirror(schema, seam)
        .inspect_err(|error| {
            tracing::warn!("failed to watch session {what}: {error}");
        })
        .ok()
}

fn apply_locale(store: &Store) -> bool {
    let Some(tag) = language::selected(store) else {
        return false;
    };
    let before = rikka_i18n::current_locale();
    match rikka_i18n::set_language(&tag) {
        Some(locale) => locale != before,
        None => {
            tracing::debug!("no catalog for `{tag}`, keeping current locale");
            false
        }
    }
}

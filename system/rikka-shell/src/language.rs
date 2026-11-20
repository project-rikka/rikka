use rikka_i18n::set_language;
use rikka_settings::Schema;
use rikka_settings::session::language;
use shell_kit::Shell;

pub(super) fn schema() -> Schema {
    language::schema()
}

pub(super) fn follow(shell: Shell) -> Shell {
    let mut catalogs = rikka_i18n::embedded!()
        .embedded(rikka_widgets::CATALOGS)
        .embedded(rikka_chat::CATALOGS)
        .embedded(rikka_llm_accounts::CATALOGS)
        .embedded(rikka_usage_bar::CATALOGS)
        .embedded(rikka_color_formats::CATALOGS);
    if let Some(chosen) = chosen(&shell) {
        catalogs = catalogs.prefer(chosen);
    }
    if let Err(error) = catalogs.install() {
        tracing::warn!("failed to load translations: {error}");
    }

    shell.on_setting(language::DOMAIN, language::LANGUAGE, |_, tag: String| {
        if set_language(&tag).is_none() {
            tracing::debug!("no translation for `{tag}`, keeping current language");
        }
    })
}

fn chosen(shell: &Shell) -> Option<String> {
    shell.reader(language::DOMAIN).and_then(|settings| {
        let store = settings.borrow();
        language::selected(store.store())
    })
}

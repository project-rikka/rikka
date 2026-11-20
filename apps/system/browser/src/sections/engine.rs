use browser_engine_settings::EngineSettings;
use rikka_app::prelude::*;
use rikka_types::Choice;

use crate::menus::profile_name;
use crate::shell::Shell;
use browser_model::ProfileId;

pub(crate) const CHOICE: f32 = 260.0;

pub(crate) fn edited_profile(shell: &Shell) -> Option<ProfileId> {
    let key = shell.model().profile_here(shell.id());
    shell.browser().settings_profile(key)
}

pub(crate) fn current(shell: &Shell) -> EngineSettings {
    let key = shell.model().profile_here(shell.id());
    shell.browser().engine_settings(key)
}

pub(crate) fn change(shell: &Shell, change: impl FnOnce(&mut EngineSettings)) {
    if let Some(profile) = edited_profile(shell) {
        shell.browser().change_engine_settings(profile, change);
    }
}

pub(crate) fn title(shell: &Shell, title: Label) -> Label {
    let model = shell.model();
    let profiles = model.profiles.list();
    let named = edited_profile(shell)
        .filter(|_| profiles.len() > 1)
        .and_then(|profile| model.profiles.get(profile))
        .map(|profile| profile_name(&profile.name));
    match named {
        Some(profile) => tr!(
            "browser.engine.titled",
            title = title.as_str(),
            profile = &profile
        ),
        None => title,
    }
}

pub(crate) fn choice_field<C: Choice>(
    label: Label,
    chosen: C,
    name: fn(C) -> Label,
    apply: fn(&mut EngineSettings, C),
) -> Element {
    field(
        label,
        combo(C::ALL.iter().position(|candidate| *candidate == chosen))
            .options(C::ALL.iter().copied().map(name))
            .on_select(move |shell: &mut Shell, at: usize| {
                if let Some(choice) = C::ALL.get(at).copied() {
                    change(shell, |settings| apply(settings, choice));
                }
            })
            .width(CHOICE),
    )
    .any()
}

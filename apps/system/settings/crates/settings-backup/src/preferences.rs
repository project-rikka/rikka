use rikka_backup::Preference;
use settings_declared::Survey;

#[must_use]
pub fn preferences(survey: &Survey) -> Vec<Preference> {
    survey
        .settings()
        .iter()
        .filter(|setting| !setting.is_locked())
        .filter_map(|setting| {
            let value = serde_json::to_value(setting.user.as_ref()?)
                .expect("settings values serialize to JSON");
            Some(Preference {
                domain: setting.id.domain.clone(),
                name: setting.id.name.clone(),
                value,
            })
        })
        .collect()
}

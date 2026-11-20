use rikka_xdg::DesktopEntry;

#[must_use]
pub fn owner<'a>(entries: &'a [DesktopEntry], app_id: &str) -> Option<&'a DesktopEntry> {
    if app_id.is_empty() {
        return None;
    }
    let by = |claims: fn(&DesktopEntry, &str) -> bool| {
        entries.iter().find(|entry| claims(entry, app_id))
    };
    let named = format!("{app_id}.desktop");
    entries
        .iter()
        .find(|entry| entry.id == named)
        .or_else(|| by(|entry, app_id| same(stem(&entry.id), app_id)))
        .or_else(|| {
            by(|entry, app_id| {
                entry
                    .startup_wm_class
                    .as_deref()
                    .is_some_and(|class| same(class, app_id))
            })
        })
        .or_else(|| by(|entry, app_id| same(tail(stem(&entry.id)), tail(app_id))))
        .or_else(|| by(|entry, app_id| same(&entry.name, app_id)))
        .or_else(|| by(|entry, app_id| entry.program().is_some_and(|p| same(&p, app_id))))
}

fn stem(id: &str) -> &str {
    id.strip_suffix(".desktop").unwrap_or(id)
}

fn tail(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn same(one: &str, other: &str) -> bool {
    one.eq_ignore_ascii_case(other)
}

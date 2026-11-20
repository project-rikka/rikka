pub const INTER: &str = "Inter";
pub const INTER_DISPLAY: &str = "Inter Display";
pub const NOTO_SANS_JP: &str = "Noto Sans CJK JP";
pub const NOTO_SANS_KR: &str = "Noto Sans CJK KR";
pub const NOTO_SANS_SC: &str = "Noto Sans CJK SC";
pub const JETBRAINS_MONO: &str = "JetBrains Mono";
pub const NOTO_COLOR_EMOJI: &str = "Noto Color Emoji";

#[must_use]
pub fn installed_families() -> Vec<String> {
    let mut names: Vec<String> = rikka_text_catalog::catalog()
        .families()
        .map(str::to_owned)
        .collect();
    names.sort_by_cached_key(|name| (name.to_lowercase(), name.clone()));
    names.dedup();
    names
}

#[must_use]
pub fn is_installed_family(name: &str) -> bool {
    rikka_text_catalog::catalog()
        .family_faces(name)
        .next()
        .is_some()
}

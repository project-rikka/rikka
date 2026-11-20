use rikka_theme::{Theme, builtin_themes};
use rikka_types::Color;

use crate::support::{contrast_ratio, for_each_generated};

fn assert_paired_roles_contrast(name: &str, theme: &Theme) {
    let palette = theme.opaque_palette();
    for (role, fill, on) in [
        ("accent", palette.accent, palette.on_accent),
        ("selection", palette.selection, palette.on_selection),
        ("danger", palette.danger, palette.on_danger),
        ("warning", palette.warning, palette.on_warning),
        ("success", palette.success, palette.on_success),
    ] {
        let ratio = contrast_ratio(fill, on);
        assert!(ratio >= 4.5, "`{name}` {role} contrast is {ratio:.2}");
    }
}

#[test]
fn builtin_themes_meet_contrast() {
    for spec in builtin_themes() {
        let theme = Theme::from_spec(spec);
        assert_paired_roles_contrast(&spec.slug, &theme);
        for fill in [
            theme.palette().surface,
            theme.palette().surface_variant,
            theme.palette().accent,
            theme.palette().danger,
        ] {
            let ratio = contrast_ratio(fill, theme.foreground_on(fill));
            assert!(ratio >= 4.5, "`{}` contrast is {ratio:.2}", spec.slug);
        }
    }
}

#[test]
fn generated_themes_meet_contrast() {
    for_each_generated(|spec| {
        assert_paired_roles_contrast(&spec.slug, &Theme::from_spec(spec));
        let colors = &spec.colors;
        for (layer, fill) in [
            ("base", colors.base),
            ("surface", colors.surface),
            ("overlay", colors.overlay),
        ] {
            let ratio = contrast_ratio(fill, colors.text);
            assert!(
                ratio >= 4.5,
                "`{}` text on {layer} is {ratio:.2}",
                spec.slug
            );
        }
    });
}

#[test]
fn accent_change_updates_foreground() {
    for spec in builtin_themes() {
        for accent in [
            Color::hex(0xffff00),
            Color::hex(0x000000),
            Color::hex(0xffffff),
            Color::hex(0x7f7f7f),
        ] {
            let theme = Theme::from_spec(spec).with_accent(accent);
            assert_eq!(theme.palette().accent, accent);

            let ratio = contrast_ratio(accent, theme.palette().on_accent);
            assert!(
                ratio >= 4.5,
                "`{}` with accent {accent:?} contrasts {ratio:.2}",
                spec.slug
            );
        }
    }
}

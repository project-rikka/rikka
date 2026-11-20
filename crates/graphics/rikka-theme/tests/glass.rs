use rikka_theme::{MaterialYou, Theme, ThemeSpec, Translucency, builtin_themes};
use rikka_types::{Color, Lens};

use crate::support::theme_toml;

fn solid_specs() -> Vec<ThemeSpec> {
    builtin_themes()
        .iter()
        .cloned()
        .chain(MaterialYou::new(Color::hex(0x7aa2f7)).specs())
        .collect()
}

#[test]
fn glass_theme_translucent_fills_opaque_text() {
    for spec in solid_specs() {
        let theme = Theme::from_spec(&spec.clone().glass());
        let glazed = theme.palette();
        assert!(theme.translucency().is_translucent(), "{}", spec.slug);
        assert!(glazed.background.a < 1.0, "background is translucent");
        for (role, ink) in [
            ("text", glazed.text),
            ("accent", glazed.accent),
            ("danger", glazed.danger),
            ("on_selection", glazed.on_selection),
        ] {
            assert_eq!(ink.a, 1.0, "`{role}` gave up some of itself");
        }
        assert!(glazed.background.a < glazed.surface.a);
        assert!(glazed.surface.a < glazed.selection.a);

        let solid = Theme::from_spec(&spec);
        assert_eq!(
            solid.palette(),
            solid.opaque_palette(),
            "solid theme palette unchanged"
        );
        assert_eq!(
            theme.opaque_palette().background,
            solid.opaque_palette().background
        );
        assert_eq!(theme.opaque_palette().text, solid.opaque_palette().text);
    }
}

#[test]
fn liquid_theme_lens_requires_blur() {
    let session = Translucency::new(1.0).with_blur(32.0);
    for spec in solid_specs() {
        let theme = Theme::from_spec(&spec.clone().liquid()).with_translucency(session);
        assert!(theme.translucency().lens().is_some(), "{}", spec.slug);
        assert!(theme.palette().background.a < 1.0);
        assert_eq!(theme.palette().text.a, 1.0);

        let glass = Theme::from_spec(&spec.glass()).with_translucency(session);
        assert_eq!(glass.translucency().lens(), None);
        assert!(theme.palette().background.a < glass.palette().background.a);
    }
}

#[test]
fn glass_keeps_clearer_theme_opacity_and_lens() {
    let clear = ThemeSpec::from_toml_str(&format!(
        "opacity = 0.1\n{}\n[lens]\nbevel = 20.0\n",
        theme_toml("clear", "#123456")
    ))
    .expect("valid theme");

    assert_eq!(clear.clone().glass().opacity, 0.1);
    let liquid = clear.liquid();
    assert_eq!(liquid.opacity, 0.1);
    assert_eq!(liquid.lens.expect("lens kept").bevel(), 20.0);
}

#[test]
fn session_translucency_multiplies_opacity_and_sets_blur() {
    let theme = Translucency::new(0.4);
    assert_eq!(theme.combine(Translucency::opaque()).opacity(), 0.4);
    assert!((theme.combine(Translucency::new(0.5)).opacity() - 0.2).abs() < 1e-6);
    assert_eq!(
        Translucency::opaque()
            .combine(Translucency::new(0.5))
            .opacity(),
        0.5
    );

    let blurred = theme.with_blur(64.0);
    assert_eq!(blurred.combine(Translucency::opaque()).blur_radius(), None);
    assert_eq!(
        blurred
            .combine(Translucency::new(1.0).with_blur(24.0))
            .blur_radius(),
        Some(24.0)
    );
    assert_eq!(Translucency::opaque().with_blur(32.0).blur_radius(), None);

    let with_lens = theme.with_lens(Some(Lens::new()));
    let session = Translucency::new(1.0).with_blur(32.0);
    assert_eq!(with_lens.combine(session).lens(), Some(Lens::new()));
    assert_eq!(Translucency::opaque().combine(session).lens(), None);
    assert_eq!(with_lens.lens(), None, "no blur to bend");
}

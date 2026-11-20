use rikka_theme::winui::{self, AccentPalette, SystemColors};
use rikka_theme::{Theme, Variant};
use rikka_types::Color;

use crate::support::contrast_ratio;

const DEFAULT_REGISTRY_PALETTE: [u8; 32] = [
    0x99, 0xeb, 0xff, 0x00, 0x4c, 0xc2, 0xff, 0x00, 0x00, 0x91, 0xf8, 0x00, 0x00, 0x78, 0xd4, 0x00,
    0x00, 0x67, 0xc0, 0x00, 0x00, 0x3e, 0x92, 0x00, 0x00, 0x1a, 0x68, 0x00, 0xf7, 0x63, 0x0c, 0x00,
];

const ACCENTS: [u32; 8] = [
    0x0078d4, 0xffb900, 0x107c10, 0xe81123, 0x881798, 0x767676, 0xffffff, 0x000000,
];

fn palettes() -> impl Iterator<Item = AccentPalette> {
    std::iter::once(AccentPalette::WINDOWS_DEFAULT).chain(
        ACCENTS
            .into_iter()
            .map(|accent| AccentPalette::from_accent(Color::hex(accent))),
    )
}

fn themes() -> impl Iterator<Item = (String, Theme)> {
    palettes().flat_map(|accent| {
        [Variant::Light, Variant::Dark]
            .into_iter()
            .map(move |variant| {
                (
                    format!("{variant:?} {}", accent.accent.to_hex()),
                    winui::theme(variant, &accent),
                )
            })
    })
}

#[test]
fn default_accent_maps_to_published_winui_roles() {
    for (variant, background, text, accent, on_accent, danger) in [
        (
            Variant::Light,
            "#f3f3f3",
            "#1a1a1a",
            "#0067c0",
            "#ffffff",
            "#c42b1c",
        ),
        (
            Variant::Dark,
            "#202020",
            "#ffffff",
            "#4cc2ff",
            "#000000",
            "#ff99a4",
        ),
    ] {
        let theme = winui::theme(variant, &AccentPalette::WINDOWS_DEFAULT);
        let palette = theme.palette();
        assert_eq!(theme.is_dark(), variant == Variant::Dark);
        assert_eq!(palette.background.to_hex(), background, "{variant:?}");
        assert_eq!(palette.text.to_hex(), text, "{variant:?}");
        assert_eq!(palette.accent.to_hex(), accent, "{variant:?}");
        assert_eq!(palette.on_accent.to_hex(), on_accent, "{variant:?}");
        assert_eq!(palette.danger.to_hex(), danger, "{variant:?}");
    }
}

#[test]
fn registry_palette_reads_shades_in_windows_order() {
    let palette = AccentPalette::from_registry(&DEFAULT_REGISTRY_PALETTE).expect("palette");
    assert_eq!(palette, AccentPalette::WINDOWS_DEFAULT);
    assert_eq!(
        AccentPalette::from_registry(&DEFAULT_REGISTRY_PALETTE[..27]),
        None
    );
}

#[test]
fn colorref_keeps_red_in_low_byte() {
    for value in [0x00d4_7800, 0xffd4_7800] {
        assert_eq!(winui::from_colorref(value).to_hex(), "#0078d4");
    }
}

#[test]
fn derived_palette_orders_shades_by_lightness() {
    let lightness = |color: Color| contrast_ratio(color, Color::BLACK);
    for accent in ACCENTS
        .into_iter()
        .filter(|&accent| accent != 0xffffff && accent != 0)
    {
        let palette = AccentPalette::from_accent(Color::hex(accent));
        let shades = [
            palette.light3,
            palette.light2,
            palette.light1,
            palette.accent,
            palette.dark1,
            palette.dark2,
            palette.dark3,
        ];
        for pair in shades.windows(2) {
            assert!(
                lightness(pair[0]) > lightness(pair[1]),
                "{accent:06x}: {} is not lighter than {}",
                pair[0].to_hex(),
                pair[1].to_hex()
            );
        }
    }
}

#[test]
fn winui_themes_meet_contrast() {
    for (name, theme) in themes() {
        let palette = theme.opaque_palette();
        for (role, fill, on) in [
            ("text", palette.background, palette.text),
            ("muted text", palette.background, palette.text_muted),
            ("surface text", palette.surface, palette.text),
            ("variant text", palette.surface_variant, palette.text),
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
}

#[test]
fn fluent_themes_are_opaque() {
    for (name, theme) in themes() {
        assert_eq!(theme.palette(), theme.opaque_palette(), "{name}");
        assert!(!theme.translucency().is_translucent(), "{name}");
    }
}

#[test]
fn high_contrast_wears_system_colors() {
    let black = SystemColors {
        window: Color::hex(0x000000),
        window_text: Color::hex(0xffffff),
        highlight: Color::hex(0x1aebff),
        highlight_text: Color::hex(0x000000),
        hotlight: Color::hex(0x75e9fc),
        gray_text: Color::hex(0xa6a6a6),
        button_face: Color::hex(0x000000),
        button_text: Color::hex(0xffffff),
    };
    let white = SystemColors {
        window: Color::hex(0xffffff),
        window_text: Color::hex(0x000000),
        highlight: Color::hex(0x37006e),
        highlight_text: Color::hex(0xffffff),
        hotlight: Color::hex(0x00009f),
        gray_text: Color::hex(0x600000),
        button_face: Color::hex(0xffffff),
        button_text: Color::hex(0x000000),
    };
    for (colors, dark) in [(black, true), (white, false)] {
        let theme = winui::high_contrast(&colors);
        let palette = theme.palette();
        assert_eq!(theme.is_dark(), dark);
        assert!(!theme.translucency().is_translucent());
        assert_eq!(palette.background, colors.window);
        assert_eq!(palette.text, colors.window_text);
        assert_eq!(palette.accent, colors.highlight);
        assert_eq!(palette.on_accent, colors.highlight_text);
        assert_eq!(palette.selection, colors.highlight);
        assert_eq!(palette.on_selection, colors.highlight_text);
        assert_eq!(palette.surface_variant, colors.button_face);
        assert_eq!(palette.outline, colors.button_text);
    }
}

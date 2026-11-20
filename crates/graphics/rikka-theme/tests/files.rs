use std::collections::{BTreeMap, BTreeSet};

use rikka_theme::{Error, MaterialYou, Theme, ThemeRegistry, ThemeSpec, Variant, builtin_themes};
use rikka_types::{Color, Lens};

use crate::support::{for_each_generated, theme_toml};

#[test]
fn parses_complete_theme_ignoring_unknown_keys() {
    let spec = ThemeSpec::from_toml_str(&format!(
        "future_key = 1\nopacity = 0.42\n{}\n[lens]\nbevel = 20.0\ndepth = 50.0\nindex = 2.0\n",
        theme_toml("sample", "#123456")
    ))
    .expect("complete theme parses");

    assert_eq!(spec.name, "Sample");
    assert_eq!(spec.slug, "sample");
    assert_eq!(spec.variant, Variant::Dark);
    assert_eq!(spec.author, None);
    assert_eq!(spec.opacity, 0.42);
    assert_eq!(spec.colors.accent, Color::rgb8(0x12, 0x34, 0x56));
    let lens = spec.lens.expect("named lens");
    assert_eq!(lens.bevel(), 20.0);
    assert_eq!(lens.depth(), 50.0);
    assert_eq!(lens.refraction_strength(), 0.5);
    assert_eq!(lens.dispersion(), Lens::new().dispersion());

    let plain = ThemeSpec::from_toml_str(&theme_toml("sample", "#123456")).expect("valid theme");
    assert_eq!(plain.opacity, 1.0);
    assert_eq!(plain.lens, None);
}

#[test]
fn rejects_missing_or_invalid_values() {
    let sample = theme_toml("sample", "#123456");
    for toml in [
        "name = \"X\"\nslug = \"x\"\nvariant = \"dark\"\n[colors]\n".to_owned(),
        sample.replace("#123456", "not-a-color"),
    ] {
        assert!(matches!(
            ThemeSpec::from_toml_str(&toml),
            Err(Error::Parse(_))
        ));
    }
    for slug in ["", "Has Space", "UPPER", "../escape"] {
        assert!(
            matches!(
                ThemeSpec::from_toml_str(&theme_toml(slug, "#123456")),
                Err(Error::InvalidSlug(_))
            ),
            "{slug:?}"
        );
    }
    assert!(matches!(
        ThemeSpec::from_toml_str(&format!("opacity = 1.5\n{sample}")),
        Err(Error::InvalidOpacity(_))
    ));
}

#[test]
fn builtin_themes_have_unique_slugs() {
    let themes = builtin_themes();
    let slugs: BTreeSet<_> = themes.iter().map(|theme| theme.slug.as_str()).collect();
    assert_eq!(slugs.len(), themes.len());

    let registry = ThemeRegistry::embedded();
    assert_eq!(registry.len(), themes.len());
    for theme in themes {
        assert_eq!(registry.get(&theme.slug), Some(theme));
    }
}

#[test]
fn theme_dir_overrides_builtins_and_reports_errors() {
    let dir = tempfile::tempdir().expect("create theme dir");
    let builtin_slug = builtin_themes()[0].slug.clone();
    let write = |name: &str, contents: &str| {
        std::fs::write(dir.path().join(name), contents).expect("write theme file");
    };
    write("override.toml", &theme_toml(&builtin_slug, "#ff00aa"));
    write("extra.toml", &theme_toml("extra", "#00aaff"));
    write("broken.toml", "name = ");
    write("notes.txt", "not a theme");

    let mut registry = ThemeRegistry::embedded();
    let report = registry.load_dir(dir.path()).expect("theme dir loads");

    assert_eq!(
        report.loaded,
        vec!["extra".to_owned(), builtin_slug.clone()]
    );
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].path, dir.path().join("broken.toml"));
    assert_eq!(
        registry.get(&builtin_slug).expect("theme overridden").name,
        "Sample"
    );
    assert!(registry.get("extra").is_some());
    assert_eq!(registry.len(), builtin_themes().len() + 1);

    let mut empty = ThemeRegistry::new();
    let report = empty
        .load_dir(dir.path().join("missing"))
        .expect("missing dir loads empty");
    assert!(report.loaded.is_empty() && report.errors.is_empty());
    assert!(empty.is_empty());
}
#[test]
fn resolve_matches_discover() {
    let folder = rikka_dirs::Paths::resolve()
        .expect("create data dir")
        .themes_dir();
    std::fs::create_dir_all(&folder).expect("create theme dir");
    let builtin_slug = builtin_themes()[0].slug.clone();
    let write = |name: &str, contents: &str| {
        std::fs::write(folder.join(name), contents).expect("write theme file");
    };
    write("a-override.toml", &theme_toml(&builtin_slug, "#ff00aa"));
    write("b-override.toml", &theme_toml(&builtin_slug, "#aa00ff"));
    write(
        "generated.toml",
        &theme_toml(MaterialYou::DARK_SLUG, "#00ffaa"),
    );
    write("extra.toml", &theme_toml("extra", "#00aaff"));
    write("broken.toml", "name = ");

    let material = MaterialYou::new(Color::hex(0x3366cc));
    let registry = ThemeRegistry::discover(material).expect("themes load");
    let slugs = registry
        .iter()
        .map(|spec| spec.slug.clone())
        .chain(["missing".to_owned()]);
    for slug in slugs {
        assert_eq!(
            ThemeRegistry::resolve(&slug, material).expect("themes load"),
            registry.get(&slug).cloned(),
            "{slug}"
        );
    }
}

#[test]
fn empty_or_unknown_slug_names_default_theme() {
    let material = MaterialYou::default();
    for slug in ["", "missing"] {
        assert_eq!(
            ThemeRegistry::named(slug, material).slug,
            Theme::LIGHT_SLUG,
            "{slug:?}"
        );
    }
    assert_eq!(
        ThemeRegistry::named(Theme::DARK_SLUG, material).slug,
        Theme::DARK_SLUG
    );
    assert_eq!(
        Theme::default(),
        Theme::from_spec(&ThemeRegistry::named("", material))
    );
    assert!(!Theme::light().is_dark());
    assert!(Theme::dark().is_dark());
}

#[derive(serde::Serialize)]
struct Written {
    name: String,
    slug: String,
    variant: &'static str,
    opacity: f32,
    colors: BTreeMap<&'static str, String>,
}

impl Written {
    fn from_spec(spec: &ThemeSpec) -> Written {
        let hex = Color::to_hex;
        let c = &spec.colors;
        Written {
            name: spec.name.clone(),
            slug: spec.slug.clone(),
            variant: match spec.variant {
                Variant::Dark => "dark",
                Variant::Light => "light",
            },
            opacity: spec.opacity,
            colors: [
                ("crust", hex(c.crust)),
                ("mantle", hex(c.mantle)),
                ("base", hex(c.base)),
                ("surface", hex(c.surface)),
                ("overlay", hex(c.overlay)),
                ("text", hex(c.text)),
                ("subtext", hex(c.subtext)),
                ("muted", hex(c.muted)),
                ("border", hex(c.border)),
                ("cursor", hex(c.cursor)),
                ("highlight", hex(c.highlight)),
                ("link", hex(c.link)),
                ("accent", hex(c.accent)),
                ("red", hex(c.red)),
                ("orange", hex(c.orange)),
                ("yellow", hex(c.yellow)),
                ("green", hex(c.green)),
                ("cyan", hex(c.cyan)),
                ("blue", hex(c.blue)),
                ("purple", hex(c.purple)),
                ("magenta", hex(c.magenta)),
                ("error", hex(c.error)),
                ("warning", hex(c.warning)),
                ("success", hex(c.success)),
                ("info", hex(c.info)),
            ]
            .into_iter()
            .collect(),
        }
    }
}

#[test]
fn generated_theme_toml_round_trips() {
    for_each_generated(|spec| {
        let written = toml::to_string(&Written::from_spec(spec)).expect("serializes");
        assert!(
            ThemeSpec::from_toml_str(&written).is_ok(),
            "`{}` does not round-trip",
            spec.slug
        );
    });
}

use rikka_material_color::Scheme;
use rikka_types::Color;

use crate::colors::ThemeColors;
use crate::hues;
use crate::material::style::MaterialStyle;
use crate::spec::ThemeSpec;
use crate::tone::{from_rgb as color, rgb};
use crate::variant::Variant;

const FALLBACK_SOURCE: u32 = 0x87bce1;

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct MaterialYou {
    source: Color,
    style: MaterialStyle,
    contrast: f32,
}

impl MaterialYou {
    pub const DARK_SLUG: &'static str = "material-you";

    pub const LIGHT_SLUG: &'static str = "material-you-light";

    pub fn new(source: Color) -> MaterialYou {
        MaterialYou {
            source,
            style: MaterialStyle::default(),
            contrast: 0.0,
        }
    }

    pub fn style(mut self, style: MaterialStyle) -> MaterialYou {
        self.style = style;
        self
    }

    pub fn contrast(mut self, contrast: f32) -> MaterialYou {
        self.contrast = contrast.clamp(-1.0, 1.0);
        self
    }

    #[must_use]
    pub fn names(slug: &str) -> bool {
        slug == MaterialYou::DARK_SLUG || slug == MaterialYou::LIGHT_SLUG
    }

    pub fn specs(&self) -> [ThemeSpec; 2] {
        [self.spec(Variant::Dark), self.spec(Variant::Light)]
    }

    pub(crate) fn named(&self, slug: &str) -> Option<ThemeSpec> {
        match slug {
            MaterialYou::DARK_SLUG => Some(self.spec(Variant::Dark)),
            MaterialYou::LIGHT_SLUG => Some(self.spec(Variant::Light)),
            _ => None,
        }
    }

    pub fn spec(&self, variant: Variant) -> ThemeSpec {
        let scheme = Scheme::new(
            rgb(self.source),
            self.style.scheme(),
            variant.is_dark(),
            f64::from(self.contrast),
        );

        let (name, slug) = match variant {
            Variant::Dark => ("Material You", MaterialYou::DARK_SLUG),
            Variant::Light => ("Material You Light", MaterialYou::LIGHT_SLUG),
        };
        ThemeSpec {
            name: name.to_owned(),
            slug: slug.to_owned(),
            variant,
            author: None,
            opacity: 1.0,
            lens: None,
            colors: colors(&scheme, self.source, variant),
        }
    }
}

impl Default for MaterialYou {
    fn default() -> MaterialYou {
        MaterialYou::new(Color::hex(FALLBACK_SOURCE))
    }
}

fn colors(scheme: &Scheme, source: Color, variant: Variant) -> ThemeColors {
    let hues = hues::harmonized(source, variant);
    ThemeColors {
        crust: color(scheme.surface_container_lowest),
        mantle: color(scheme.surface),
        base: color(scheme.surface_container_low),
        surface: color(scheme.surface_container),
        overlay: color(scheme.surface_container_high),
        text: color(scheme.on_surface),
        subtext: color(scheme.on_surface_variant),
        muted: color(scheme.outline),
        border: color(scheme.outline_variant),
        cursor: color(scheme.primary),
        highlight: color(scheme.surface_container_highest),
        link: color(scheme.primary),
        accent: color(scheme.primary),
        red: hues.red,
        orange: hues.orange,
        yellow: hues.yellow,
        green: hues.green,
        cyan: hues.cyan,
        blue: hues.blue,
        purple: hues.purple,
        magenta: hues.magenta,
        error: color(scheme.error),
        warning: hues.orange,
        success: hues.green,
        info: hues.blue,
    }
}

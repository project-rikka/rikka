use rikka_types::{Color, srgb};

use crate::ansi::{Ansi, Hues, Ink};
use crate::builtin::{self, builtin_theme};
use crate::palette::Palette;
use crate::spec::ThemeSpec;
use crate::surfaces::Surfaces;
use crate::syntax::Syntax;
use crate::translucency::Translucency;
use crate::typography::Typography;
use crate::variant::Variant;

const HOVER_ALPHA: f32 = 0.08;
const PRESS_ALPHA: f32 = 0.14;

#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct Theme {
    opaque_palette: Palette,
    palette: Palette,
    translucency: Translucency,
    ansi: Ansi,
    syntax: Syntax,
    variant: Variant,
    typography: Typography,
}

impl Theme {
    pub const LIGHT_SLUG: &'static str = builtin::LIGHT_SLUG;

    pub const DARK_SLUG: &'static str = builtin::DARK_SLUG;

    pub fn from_spec(spec: &ThemeSpec) -> Self {
        let c = &spec.colors;
        let variant = spec.variant;
        let surfaces = Surfaces::derive(c.base, c.surface, c.overlay, variant);
        let selection = surfaces.selection(c.accent);
        let translucency = Translucency::new(spec.opacity).with_lens(spec.lens);
        let hues = Hues {
            red: c.red,
            orange: c.orange,
            green: c.green,
            yellow: c.yellow,
            blue: c.blue,
            magenta: c.magenta,
            purple: c.purple,
            cyan: c.cyan,
        };
        let ink = Ink {
            background: surfaces.background,
            text: c.text,
            subtext: c.subtext,
            muted: c.muted,
            cursor: c.cursor,
        };
        let opaque_palette = Palette {
            background: surfaces.background,
            surface: surfaces.surface,
            surface_variant: surfaces.surface_variant,
            text: c.text,
            text_muted: c.muted,
            accent: c.accent,
            on_accent: contrasting_text(c.accent, c.crust, c.text),
            selection,
            on_selection: contrasting_text(selection, c.crust, c.text),
            danger: c.error,
            on_danger: contrasting_text(c.error, c.crust, c.text),
            warning: c.warning,
            on_warning: contrasting_text(c.warning, c.crust, c.text),
            success: c.success,
            on_success: contrasting_text(c.success, c.crust, c.text),
            outline: c.border,
            overlay: overlay(variant),
        };
        Self::assemble(opaque_palette, translucency, &hues, &ink, variant)
    }

    pub(crate) fn assemble(
        opaque_palette: Palette,
        translucency: Translucency,
        hues: &Hues,
        ink: &Ink,
        variant: Variant,
    ) -> Self {
        Self {
            opaque_palette,
            palette: translucency.derive(opaque_palette),
            translucency,
            ansi: Ansi::derive(hues, ink, variant),
            syntax: Syntax::derive(hues, ink, variant),
            variant,
            typography: Typography::default(),
        }
    }

    pub fn light() -> Self {
        Self::from_spec(builtin_theme(Self::LIGHT_SLUG))
    }

    pub fn dark() -> Self {
        Self::from_spec(builtin_theme(Self::DARK_SLUG))
    }

    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.variant.is_dark()
    }

    pub(crate) fn variant(&self) -> Variant {
        self.variant
    }

    #[inline]
    #[must_use]
    pub fn palette(&self) -> Palette {
        self.palette
    }

    pub fn ansi(&self) -> Ansi {
        self.ansi
    }

    pub fn syntax(&self) -> Syntax {
        self.syntax
    }

    pub fn translucency(&self) -> Translucency {
        self.translucency
    }

    pub fn opaque_palette(&self) -> Palette {
        self.opaque_palette
    }

    pub fn typography(&self) -> &Typography {
        &self.typography
    }

    pub fn with_accent(mut self, accent: Color) -> Self {
        self.opaque_palette.on_accent = self.foreground_on(accent);
        self.opaque_palette.accent = accent;
        self.opaque_palette.selection = Surfaces::derive(
            self.opaque_palette.background,
            self.opaque_palette.surface,
            self.opaque_palette.surface_variant,
            self.variant,
        )
        .selection(accent);
        self.opaque_palette.on_selection = self.foreground_on(self.opaque_palette.selection);
        self.update_palette()
    }

    pub fn with_translucency(mut self, session: Translucency) -> Self {
        self.translucency = self.translucency.combine(session);
        self.update_palette()
    }

    pub(crate) fn glazed(mut self, translucency: Translucency, outline: Color) -> Self {
        self.opaque_palette.outline = outline;
        self.translucency = translucency;
        self.update_palette()
    }

    fn update_palette(mut self) -> Self {
        self.palette = self.translucency.derive(self.opaque_palette);
        self
    }

    pub fn foreground_on(&self, fill: Color) -> Color {
        contrasting_text(
            fill,
            self.opaque_palette.background,
            self.opaque_palette.text,
        )
    }

    pub fn hovered(&self, base: Color) -> Color {
        self.layered(base, HOVER_ALPHA)
    }

    pub fn pressed(&self, base: Color) -> Color {
        self.layered(base, PRESS_ALPHA)
    }

    pub fn hover_layer(&self) -> Color {
        self.opaque_palette.overlay.with_alpha(HOVER_ALPHA)
    }

    pub fn press_layer(&self) -> Color {
        self.opaque_palette.overlay.with_alpha(PRESS_ALPHA)
    }

    pub fn dim(&self, base: Color, disabled: bool) -> Color {
        if disabled { self.disabled(base) } else { base }
    }

    fn layered(&self, base: Color, alpha: f32) -> Color {
        if base.is_transparent() {
            self.opaque_palette.overlay.with_alpha(alpha)
        } else {
            base.mix(self.opaque_palette.overlay, alpha)
        }
    }

    pub fn disabled(&self, base: Color) -> Color {
        base.with_alpha(base.a * 0.45)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

pub(crate) fn overlay(variant: Variant) -> Color {
    match variant {
        Variant::Dark => Color::WHITE,
        Variant::Light => Color::BLACK,
    }
}

const AA: f32 = 4.5;

pub(crate) fn preferred_text(fill: Color, preferred: Color, alternative: Color) -> Color {
    if contrast_ratio(fill, preferred) >= AA {
        preferred
    } else {
        contrasting_text(fill, preferred, alternative)
    }
}

pub(crate) fn contrasting_text(fill: Color, a: Color, b: Color) -> Color {
    let ratio_a = contrast_ratio(fill, a);
    let ratio_b = contrast_ratio(fill, b);
    let (best, best_ratio) = if ratio_a >= ratio_b {
        (a, ratio_a)
    } else {
        (b, ratio_b)
    };
    if best_ratio >= AA {
        best
    } else if contrast_ratio(fill, Color::BLACK) >= contrast_ratio(fill, Color::WHITE) {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(color: Color) -> f32 {
    srgb::luminance([color.r, color.g, color.b].map(srgb::to_linear))
}

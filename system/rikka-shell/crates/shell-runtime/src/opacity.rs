use rikka_wayland_client::Route;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_alpha_modifier_surface_v1::WpAlphaModifierSurfaceV1;
use rikka_wayland_client::wp_alpha_modifier_v1::WpAlphaModifierV1;
use shell_kit::Opacity;
use shell_kit::hosting::OpacityLevels;

pub(crate) struct AlphaMultiplier {
    modifier: WpAlphaModifierSurfaceV1,
    output: String,
    sent: f32,
}

impl AlphaMultiplier {
    pub(crate) fn new(
        manager: &WpAlphaModifierV1,
        surface: &WlSurface,
        opacity: &Opacity,
        output: String,
    ) -> AlphaMultiplier {
        let modifier = manager.via(&Route::discard()).get_surface(surface);
        let sent = opacity.shown(&output);
        modifier.set_multiplier(factor(sent));
        AlphaMultiplier {
            modifier,
            output,
            sent,
        }
    }

    pub(crate) fn follow(&mut self, opacity: &Opacity) -> bool {
        let shown = opacity.shown(&self.output);
        if shown == self.sent {
            return false;
        }
        self.modifier.set_multiplier(factor(shown));
        self.sent = shown;
        true
    }

    #[must_use]
    pub(crate) fn is_clear(&self) -> bool {
        self.sent <= 0.0
    }
}

impl Drop for AlphaMultiplier {
    fn drop(&mut self) {
        self.modifier.destroy();
    }
}

pub(crate) struct Driving {
    opacity: Opacity,
    output: String,
    drawn: Option<f32>,
    sealed: Option<f32>,
}

impl Driving {
    pub(crate) fn new(opacity: Opacity, output: String) -> Driving {
        Driving {
            opacity,
            output,
            drawn: None,
            sealed: None,
        }
    }

    pub(crate) fn draw(&mut self) {
        self.drawn = Some(self.opacity.drawn(&self.output));
    }

    pub(crate) fn seal(&mut self) {
        self.sealed = self.drawn;
    }

    pub(crate) fn show(&mut self) {
        if let Some(sealed) = self.sealed.take() {
            self.opacity.show(&self.output, sealed);
        }
    }
}

impl Drop for Driving {
    fn drop(&mut self) {
        self.opacity.clear(&self.output);
    }
}

fn factor(opacity: f32) -> u32 {
    (f64::from(opacity) * f64::from(u32::MAX)).round() as u32
}

use rikka_types::PixelSize;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use rikka_wayland_client::wp_fractional_scale_v1::{self, WpFractionalScaleV1};
use rikka_wayland_client::wp_viewporter::WpViewporter;
use rikka_wayland_client::{Key, Proxy as _, Route};

use crate::display::Display;

const FRACTIONAL_SCALE_DENOMINATOR: u32 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    Integer(u32),
    Fractional(u32),
}

impl Default for Scale {
    fn default() -> Scale {
        Scale::Integer(1)
    }
}

impl Scale {
    #[must_use]
    pub fn factor(self) -> f64 {
        match self {
            Scale::Integer(factor) => f64::from(factor),
            Scale::Fractional(numerator) => {
                f64::from(numerator) / f64::from(FRACTIONAL_SCALE_DENOMINATOR)
            }
        }
    }

    #[must_use]
    pub fn physical(self, logical: PixelSize) -> PixelSize {
        let scaled = |length: u32| match self {
            Scale::Integer(factor) => length.saturating_mul(factor),
            Scale::Fractional(numerator) => {
                let scaled = (u64::from(length) * u64::from(numerator)
                    + u64::from(FRACTIONAL_SCALE_DENOMINATOR / 2))
                    / u64::from(FRACTIONAL_SCALE_DENOMINATOR);
                u32::try_from(scaled).unwrap_or(u32::MAX)
            }
        };
        PixelSize::new(scaled(logical.width).max(1), scaled(logical.height).max(1))
    }

    #[must_use]
    pub fn cursor_scale(self) -> u32 {
        self.factor().ceil().max(1.0) as u32
    }
}

#[must_use]
pub fn output_scale(mode: (i32, i32), logical: (i32, i32)) -> Option<f64> {
    let long = |(width, height): (i32, i32)| width.max(height);
    (long(mode) > 0 && long(logical) > 0).then(|| f64::from(long(mode)) / f64::from(long(logical)))
}

#[derive(Clone, Debug)]
pub struct FractionalScaleManager {
    manager: WpFractionalScaleManagerV1,
}

impl FractionalScaleManager {
    #[must_use]
    pub fn bind(display: &Display, route: &Route) -> Option<FractionalScaleManager> {
        if !display.offers(WpViewporter::INTERFACE.name) {
            tracing::debug!("no wp_viewporter, using integer scaling");
            return None;
        }
        display
            .bind_optional_on(route, 1..=1)
            .map(|manager| FractionalScaleManager { manager })
    }
}

#[derive(Debug, Default)]
pub struct Scaling {
    fractional: Option<WpFractionalScaleV1>,
    preferred_fractional: Option<u32>,
    preferred_integer: Option<u32>,
}

impl Scaling {
    pub fn new<K: Key>(
        manager: Option<&FractionalScaleManager>,
        surface: &WlSurface,
        key: K,
    ) -> Scaling {
        Scaling {
            fractional: manager
                .map(|manager| manager.manager.get_fractional_scale(surface).with_key(key)),
            preferred_fractional: None,
            preferred_integer: None,
        }
    }

    pub fn set_initial_factor(&mut self, factor: f64) {
        if self.preferred_fractional.is_none() && factor > 0.0 {
            self.preferred_fractional =
                Some((factor * f64::from(FRACTIONAL_SCALE_DENOMINATOR)).round() as u32)
                    .filter(|scale| *scale > 0);
        }
    }

    pub fn handle_event(&mut self, event: wp_fractional_scale_v1::Event) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            self.preferred_fractional = Some(scale).filter(|scale| *scale > 0);
        }
    }

    pub fn set_preferred_buffer_scale(&mut self, factor: i32) {
        self.preferred_integer = u32::try_from(factor).ok().filter(|factor| *factor > 0);
    }

    #[must_use]
    pub fn scale(&self, output_scale: impl FnOnce() -> u32) -> Scale {
        match self
            .preferred_fractional
            .filter(|_| self.fractional.is_some())
        {
            Some(numerator) => Scale::Fractional(numerator),
            None => Scale::Integer(self.preferred_integer.unwrap_or_else(output_scale).max(1)),
        }
    }
}

impl Drop for Scaling {
    fn drop(&mut self) {
        if let Some(fractional) = self.fractional.take() {
            fractional.destroy();
        }
    }
}

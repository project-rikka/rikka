use rikka_renderer_3d::{ParticleKind, Resampling, ShadowQuality, Tier};
use rikka_shader_graph::{Hint, PortSpec};

pub trait Choice: Copy + PartialEq + 'static {
    const ORDER: &'static [Self];
    const IDS: &'static [&'static str];

    #[must_use]
    fn id(self) -> &'static str {
        Self::IDS[self.index()]
    }

    #[must_use]
    fn index(self) -> usize {
        Self::ORDER
            .iter()
            .position(|choice| *choice == self)
            .expect("every choice is listed in its order")
    }

    #[must_use]
    fn named(id: &str) -> Option<Self> {
        Self::IDS
            .iter()
            .position(|known| *known == id)
            .map(|at| Self::ORDER[at])
    }

    #[must_use]
    fn picked(value: f32) -> Self {
        let last = Self::ORDER.len() - 1;
        let at = match value.is_finite() {
            true => (value.round().max(0.0) as usize).min(last),
            false => 0,
        };
        Self::ORDER[at]
    }
}

macro_rules! choices {
    ($kind:ty, [$($variant:path => $id:literal),+ $(,)?]) => {
        impl $crate::world::choice::Choice for $kind {
            const ORDER: &'static [Self] = &[$($variant),+];
            const IDS: &'static [&'static str] = &[$($id),+];
        }
    };
}

pub(crate) use choices;

pub(crate) fn options<T: Choice>(name: &'static str, chosen: T) -> PortSpec {
    PortSpec::number(name, chosen.index() as f32).hint(Hint::Options(T::IDS))
}

choices!(Tier, [
    Tier::Auto => "auto",
    Tier::Off => "off",
    Tier::ScreenSpace => "screen_space",
    Tier::RayTraced => "ray_traced",
]);

choices!(ShadowQuality, [
    ShadowQuality::None => "none",
    ShadowQuality::Low => "low",
    ShadowQuality::Medium => "medium",
    ShadowQuality::High => "high",
]);

choices!(ParticleKind, [
    ParticleKind::Motes => "motes",
    ParticleKind::Petals => "petals",
    ParticleKind::Sparkles => "sparkles",
    ParticleKind::Snow => "snow",
]);

choices!(Resampling, [
    Resampling::Off => "off",
    Resampling::Temporal => "temporal",
    Resampling::Spatial => "spatial",
    Resampling::Spatiotemporal => "spatiotemporal",
]);

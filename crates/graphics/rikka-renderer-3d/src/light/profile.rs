use glam::Quat;
use rikka_ies::IesProfile;

pub(crate) const PROFILE_VERTICAL: usize = 128;

pub(crate) const PROFILE_HORIZONTAL: usize = 64;

pub(crate) const PROFILE_TEXELS: usize = PROFILE_VERTICAL * PROFILE_HORIZONTAL;

/// A photometric profile created with
/// [`Renderer3d::create_light_profile`](crate::Renderer3d::create_light_profile).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LightProfileId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// An LM-63 intensity distribution shaping a point or spot light.
///
/// The profile is normalised to its peak, so the light's intensity is the intensity of the
/// profile's brightest direction; [`IesProfile::peak_candela`] gives it in candela for
/// [`Light::candela`](crate::Light::candela). `orientation` turns the luminaire frame (nadir
/// along −Y, horizontal angle 0° along +X, 90° along −Z) into the world. A spot light keeps
/// its direction as the nadir and takes only the 0° horizontal axis from `orientation`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Photometry {
    pub profile: LightProfileId,
    pub orientation: Quat,
}

pub(crate) fn bake(profile: &IesProfile, texels: &mut [f32]) {
    let peak = profile.peak_candela();
    let (rows, _) = texels.as_chunks_mut::<PROFILE_HORIZONTAL>();
    for (row, values) in rows.iter_mut().enumerate() {
        let vertical = row as f32 * 180.0 / (PROFILE_VERTICAL - 1) as f32;
        for (column, value) in values.iter_mut().enumerate() {
            let horizontal = column as f32 * 360.0 / PROFILE_HORIZONTAL as f32;
            *value = profile.candela(vertical, horizontal) / peak;
        }
    }
}

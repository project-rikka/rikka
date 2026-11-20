use std::sync::{Arc, OnceLock};

use moxcms::{ColorProfile, DataColorSpace, ProfileText};
use rikka_types::srgb;

use crate::error::{Error, Result};

#[derive(Clone)]
pub struct Profile {
    inner: Arc<Inner>,
}

struct Inner {
    icc: Arc<[u8]>,
    parsed: ColorProfile,
    name: Option<String>,
    srgb: bool,
}

impl Profile {
    pub fn from_icc(bytes: impl Into<Arc<[u8]>>) -> Result<Profile> {
        let icc: Arc<[u8]> = bytes.into();
        let parsed = ColorProfile::new_from_slice(&icc)
            .map_err(|error| Error::Profile(error.to_string()))?;
        let usable = matches!(
            parsed.color_space,
            DataColorSpace::Rgb | DataColorSpace::Gray
        );
        if !usable {
            return Err(Error::Profile(format!(
                "unsupported {:?} profile colour space",
                parsed.color_space
            )));
        }
        let name = parsed.description.as_ref().and_then(text);
        Ok(Profile::from_parts(icc, parsed, name))
    }

    #[must_use]
    pub fn srgb() -> Profile {
        static PROFILE: OnceLock<Profile> = OnceLock::new();
        PROFILE
            .get_or_init(|| builtin(ColorProfile::new_srgb(), "sRGB IEC61966-2.1"))
            .clone()
    }

    #[must_use]
    pub fn display_p3() -> Profile {
        static PROFILE: OnceLock<Profile> = OnceLock::new();
        PROFILE
            .get_or_init(|| builtin(ColorProfile::new_display_p3(), "Display P3"))
            .clone()
    }

    #[must_use]
    pub fn adobe_rgb() -> Profile {
        static PROFILE: OnceLock<Profile> = OnceLock::new();
        PROFILE
            .get_or_init(|| builtin(ColorProfile::new_adobe_rgb(), "Adobe RGB (1998)"))
            .clone()
    }

    #[must_use]
    pub fn pro_photo_rgb() -> Profile {
        static PROFILE: OnceLock<Profile> = OnceLock::new();
        PROFILE
            .get_or_init(|| builtin(ColorProfile::new_pro_photo_rgb(), "ProPhoto RGB"))
            .clone()
    }

    #[must_use]
    pub fn rec2020() -> Profile {
        static PROFILE: OnceLock<Profile> = OnceLock::new();
        PROFILE
            .get_or_init(|| builtin(ColorProfile::new_bt2020(), "Rec. 2020"))
            .clone()
    }

    #[must_use]
    pub fn linear_srgb() -> Profile {
        static PROFILE: OnceLock<Profile> = OnceLock::new();
        PROFILE
            .get_or_init(|| {
                let mut linear = ColorProfile::new_srgb();
                let curve = moxcms::curve_from_gamma(1.0);
                linear.red_trc = Some(curve.clone());
                linear.green_trc = Some(curve.clone());
                linear.blue_trc = Some(curve);
                builtin(linear, "Linear sRGB")
            })
            .clone()
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    #[must_use]
    pub fn icc(&self) -> Arc<[u8]> {
        Arc::clone(&self.inner.icc)
    }

    #[must_use]
    pub fn is_gray(&self) -> bool {
        self.inner.parsed.color_space == DataColorSpace::Gray
    }

    pub(crate) fn parsed(&self) -> &ColorProfile {
        &self.inner.parsed
    }

    pub(crate) fn is_srgb(&self) -> bool {
        self.inner.srgb
    }

    fn from_parts(icc: Arc<[u8]>, parsed: ColorProfile, name: Option<String>) -> Profile {
        let srgb = describes_srgb(&parsed);
        Profile {
            inner: Arc::new(Inner {
                icc,
                parsed,
                name,
                srgb,
            }),
        }
    }
}

fn describes_srgb(parsed: &ColorProfile) -> bool {
    static SRGB: OnceLock<ColorProfile> = OnceLock::new();
    let srgb = SRGB.get_or_init(ColorProfile::new_srgb);
    let close = |a: moxcms::Xyzd, b: moxcms::Xyzd| {
        (a.x - b.x).abs() < 2e-3 && (a.y - b.y).abs() < 2e-3 && (a.z - b.z).abs() < 2e-3
    };
    parsed.is_matrix_shaper()
        && close(parsed.red_colorant, srgb.red_colorant)
        && close(parsed.green_colorant, srgb.green_colorant)
        && close(parsed.blue_colorant, srgb.blue_colorant)
        && [&parsed.red_trc, &parsed.green_trc, &parsed.blue_trc]
            .into_iter()
            .all(|curve| curve.as_ref().is_some_and(follows_srgb))
}

fn follows_srgb(curve: &moxcms::ToneReprCurve) -> bool {
    let Ok(evaluator) = curve.make_linear_evaluator() else {
        return false;
    };
    (0..=32).all(|step| {
        let value = step as f32 / 32.0;
        (evaluator.evaluate_value(value) - srgb::to_linear(value)).abs() < 2e-3
    })
}

fn builtin(mut profile: ColorProfile, name: &str) -> Profile {
    profile.description = Some(ProfileText::PlainString(name.to_owned()));
    let icc: Arc<[u8]> = profile.encode().expect("built-in profile encodes").into();
    Profile::from_parts(icc, profile, Some(name.to_owned()))
}

fn text(description: &ProfileText) -> Option<String> {
    let text = match description {
        ProfileText::PlainString(plain) => plain.clone(),
        ProfileText::Localizable(strings) => strings
            .iter()
            .find(|string| string.language == "en")
            .or_else(|| strings.first())
            .map(|string| string.value.clone())?,
        ProfileText::Description(description) => match description.unicode_string.is_empty() {
            true => description.ascii_string.clone(),
            false => description.unicode_string.clone(),
        },
    };
    let trimmed = text.trim_matches(char::from(0)).trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

impl PartialEq for Profile {
    fn eq(&self, other: &Profile) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner) || self.inner.icc == other.inner.icc
    }
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profile")
            .field("name", &self.inner.name)
            .field("bytes", &self.inner.icc.len())
            .finish()
    }
}

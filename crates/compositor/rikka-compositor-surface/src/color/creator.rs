use rikka_colorimetry::{Chromaticity, Description, Luminance, Mastering, Primaries};
use rikka_compositor_protocols::wp_color_manager_v1::{
    Feature, Primaries as NamedPrimaries, TransferFunction,
};
use rikka_compositor_protocols::wp_image_description_creator_params_v1::Error;

use super::description::{Parametric, Transfer};
use super::named;
use super::support::ColorSupport;

const CHROMATICITY_SCALE: f64 = 1_000_000.0;
const MIN_LUMINANCE_SCALE: f64 = 10_000.0;
const EXPONENT_SCALE: f64 = 10_000.0;
const POWER_RANGE: std::ops::RangeInclusive<u32> = 10_000..=100_000;
const RELAXED_LIGHT_LEVELS_SINCE: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Range {
    min: f64,
    max: f64,
}

#[derive(Debug, Default)]
pub(crate) struct Creator {
    transfer: Option<Transfer>,
    primaries: Option<(Primaries, Option<NamedPrimaries>)>,
    luminance: Option<Luminance>,
    mastering_primaries: Option<Primaries>,
    mastering_luminance: Option<Range>,
    max_cll: Option<u32>,
    max_fall: Option<u32>,
}

pub(crate) enum Finished {
    Described(Box<Parametric>),
    Unsupported(&'static str),
}

impl Creator {
    pub(crate) fn transfer_named(
        &mut self,
        support: &ColorSupport,
        transfer: Option<TransferFunction>,
    ) -> Result<(), Error> {
        let transfer = transfer
            .filter(|transfer| support.transfer_functions.contains(transfer))
            .ok_or(Error::InvalidTf)?;
        set_once(&mut self.transfer, Transfer::Named(transfer))
    }

    pub(crate) fn transfer_power(
        &mut self,
        support: &ColorSupport,
        eexp: u32,
    ) -> Result<(), Error> {
        require(support, Feature::SetTfPower)?;
        if !POWER_RANGE.contains(&eexp) {
            return Err(Error::InvalidTf);
        }
        set_once(
            &mut self.transfer,
            Transfer::Power(f64::from(eexp) / EXPONENT_SCALE),
        )
    }

    pub(crate) fn primaries_named(
        &mut self,
        support: &ColorSupport,
        primaries: Option<NamedPrimaries>,
    ) -> Result<(), Error> {
        let primaries = primaries
            .filter(|primaries| support.primaries.contains(primaries))
            .ok_or(Error::InvalidPrimariesNamed)?;
        set_once(
            &mut self.primaries,
            (named::primaries(primaries), Some(primaries)),
        )
    }

    pub(crate) fn primaries(
        &mut self,
        support: &ColorSupport,
        values: [i32; 8],
    ) -> Result<(), Error> {
        require(support, Feature::SetPrimaries)?;
        set_once(&mut self.primaries, (chromaticities(values), None))
    }

    pub(crate) fn luminances(
        &mut self,
        support: &ColorSupport,
        min: u32,
        max: u32,
        reference: u32,
    ) -> Result<(), Error> {
        require(support, Feature::SetLuminances)?;
        let min = f64::from(min) / MIN_LUMINANCE_SCALE;
        let (max, reference) = (f64::from(max), f64::from(reference));
        if max <= min || reference <= min {
            return Err(Error::InvalidLuminance);
        }
        set_once(
            &mut self.luminance,
            Luminance {
                min,
                max,
                reference,
            },
        )
    }

    pub(crate) fn mastering_primaries(
        &mut self,
        support: &ColorSupport,
        values: [i32; 8],
    ) -> Result<(), Error> {
        require(support, Feature::SetMasteringDisplayPrimaries)?;
        set_once(&mut self.mastering_primaries, chromaticities(values))
    }

    pub(crate) fn mastering_luminance(
        &mut self,
        support: &ColorSupport,
        min: u32,
        max: u32,
    ) -> Result<(), Error> {
        require(support, Feature::SetMasteringDisplayPrimaries)?;
        let min = f64::from(min) / MIN_LUMINANCE_SCALE;
        let max = f64::from(max);
        if max <= min {
            return Err(Error::InvalidLuminance);
        }
        set_once(&mut self.mastering_luminance, Range { min, max })
    }

    pub(crate) fn max_cll(&mut self, value: u32) -> Result<(), Error> {
        set_once(&mut self.max_cll, value)
    }

    pub(crate) fn max_fall(&mut self, value: u32) -> Result<(), Error> {
        set_once(&mut self.max_fall, value)
    }

    pub(crate) fn finish(self, version: u32) -> Result<Finished, Error> {
        let (Some(transfer), Some((primaries, named_primaries))) = (self.transfer, self.primaries)
        else {
            return Err(Error::IncompleteSet);
        };
        let mut luminance = self
            .luminance
            .unwrap_or_else(|| named::default_luminance(transfer));
        if transfer == Transfer::Named(TransferFunction::St2084Pq) {
            named::pq_swing(&mut luminance);
        }
        let target = self.mastering_luminance.unwrap_or(Range {
            min: luminance.min,
            max: luminance.max,
        });
        if let (Some(cll), Some(fall)) = (self.max_cll, self.max_fall)
            && fall > cll
        {
            return Err(Error::InvalidLuminance);
        }
        if version < RELAXED_LIGHT_LEVELS_SINCE {
            let within = |level: u32| {
                let level = f64::from(level);
                level > target.min && level <= target.max
            };
            if !self.max_cll.is_none_or(within) || !self.max_fall.is_none_or(within) {
                return Err(Error::InvalidLuminance);
            }
        }
        let Some(function) = named::function(transfer) else {
            return Ok(Finished::Unsupported(
                "transfer function is not implemented",
            ));
        };
        let mastering_primaries = self.mastering_primaries.unwrap_or(primaries);
        if !primaries.is_valid() || !mastering_primaries.is_valid() {
            return Ok(Finished::Unsupported(
                "primaries do not span a colour space",
            ));
        }
        Ok(Finished::Described(Box::new(Parametric {
            transfer,
            named_primaries,
            description: Description {
                primaries,
                transfer: function,
                luminance,
                mastering: Some(Mastering {
                    primaries: mastering_primaries,
                    min: target.min,
                    max: target.max,
                }),
                max_cll: self.max_cll.map(f64::from),
                max_fall: self.max_fall.map(f64::from),
            },
        })))
    }
}

pub(crate) fn scaled_chromaticities(primaries: &Primaries) -> [i32; 8] {
    let scale = |value: f64| (value * CHROMATICITY_SCALE).round() as i32;
    [
        scale(primaries.red.x),
        scale(primaries.red.y),
        scale(primaries.green.x),
        scale(primaries.green.y),
        scale(primaries.blue.x),
        scale(primaries.blue.y),
        scale(primaries.white.x),
        scale(primaries.white.y),
    ]
}

pub(crate) fn scaled_min_luminance(min: f64) -> u32 {
    (min * MIN_LUMINANCE_SCALE).round() as u32
}

pub(crate) fn scaled_exponent(exponent: f64) -> u32 {
    (exponent * EXPONENT_SCALE).round() as u32
}

fn chromaticities(values: [i32; 8]) -> Primaries {
    let point = |x: i32, y: i32| {
        Chromaticity::new(
            f64::from(x) / CHROMATICITY_SCALE,
            f64::from(y) / CHROMATICITY_SCALE,
        )
    };
    Primaries::new(
        point(values[0], values[1]),
        point(values[2], values[3]),
        point(values[4], values[5]),
        point(values[6], values[7]),
    )
}

fn require(support: &ColorSupport, feature: Feature) -> Result<(), Error> {
    if support.has(Feature::Parametric) && support.has(feature) {
        Ok(())
    } else {
        Err(Error::UnsupportedFeature)
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), Error> {
    if slot.is_some() {
        return Err(Error::AlreadySet);
    }
    *slot = Some(value);
    Ok(())
}

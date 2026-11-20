use std::sync::{Arc, OnceLock};

use moxcms::{
    ColorProfile, DataColorSpace, Layout, ToneReprCurve, TransformF32Executor, TransformOptions,
};
use rikka_gpu::ComputeBuffer;
use rikka_types::srgb;

use crate::color::conversion::{Conversion, Intent, PreparedConversion};
use crate::color::profile::Profile;
use crate::error::Result;
use crate::work::Work;

pub(crate) const TABLE_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transfer {
    Identity = 0,
    Srgb = 1,
    Tables = 2,
}

pub(crate) struct Space {
    profile: Option<Profile>,
    transfer: Transfer,
    tables: Vec<f32>,
    to_xyz: [[f64; 3]; 3],
    table_buffer: OnceLock<ComputeBuffer>,
    display_conversion: OnceLock<PreparedConversion>,
    srgb_transform_cache: OnceLock<Option<(Arc<TransformF32Executor>, bool)>>,
}

impl Space {
    pub(crate) fn new(profile: Option<&Profile>) -> Space {
        let srgb = Profile::srgb();
        let parsed = profile.map(Profile::parsed);
        let to_xyz = parsed
            .filter(|parsed| parsed.is_matrix_shaper())
            .unwrap_or(srgb.parsed())
            .rgb_to_xyz_matrix()
            .v;
        let curves = parsed.and_then(curves_of);
        let (transfer, tables) = match (profile, curves) {
            (None, _) => (Transfer::Srgb, Vec::new()),
            (Some(profile), _) if profile.is_srgb() => (Transfer::Srgb, Vec::new()),
            (Some(_), Some(curves)) if curves.iter().all(is_identity) => {
                (Transfer::Identity, Vec::new())
            }
            (Some(_), Some(curves)) => match tables_of(&curves) {
                Some(tables) => (Transfer::Tables, tables),
                None => (Transfer::Srgb, Vec::new()),
            },
            (Some(_), None) => (Transfer::Srgb, Vec::new()),
        };
        Space {
            profile: profile.cloned(),
            transfer,
            tables,
            to_xyz,
            table_buffer: OnceLock::new(),
            display_conversion: OnceLock::new(),
            srgb_transform_cache: OnceLock::new(),
        }
    }

    pub(crate) fn profile(&self) -> Option<&Profile> {
        self.profile.as_ref()
    }

    pub(crate) fn transfer(&self) -> u32 {
        self.transfer as u32
    }

    pub(crate) fn display(&self, work: &Work) -> Result<PreparedConversion> {
        if let Some(prepared) = self.display_conversion.get() {
            return Ok(prepared.clone());
        }
        let prepared =
            Conversion::new(self.profile(), None, Intent::RelativeColorimetric)?.prepare(work)?;
        Ok(self.display_conversion.get_or_init(|| prepared).clone())
    }

    pub(crate) fn tables(&self, work: &Work) -> Result<ComputeBuffer> {
        if let Some(buffer) = self.table_buffer.get() {
            return Ok(buffer.clone());
        }
        let buffer = work.constants(&self.tables)?;
        Ok(self.table_buffer.get_or_init(|| buffer).clone())
    }

    pub(crate) fn to_xyz(&self) -> [[f64; 3]; 3] {
        self.to_xyz
    }

    pub(crate) fn linear_premultiplied(&self, color: [f32; 4]) -> [f32; 4] {
        let [r, g, b, a] = self.translated(color);
        let linear = [r, g, b]
            .map(|value| value.clamp(0.0, 1.0))
            .into_iter()
            .enumerate()
            .map(|(channel, value)| match self.transfer {
                Transfer::Identity => value,
                Transfer::Srgb => srgb::to_linear(value),
                Transfer::Tables => {
                    let position = value * (TABLE_SIZE - 1) as f32;
                    let low = (position as usize).min(TABLE_SIZE - 2);
                    let t = position - low as f32;
                    let at = channel * TABLE_SIZE + low;
                    self.tables[at] + (self.tables[at + 1] - self.tables[at]) * t
                }
            });
        let mut out = [0.0; 4];
        for (slot, value) in out.iter_mut().zip(linear) {
            *slot = value * a;
        }
        out[3] = a;
        out
    }

    pub(crate) fn translated(&self, color: [f32; 4]) -> [f32; 4] {
        let Some((transform, gray)) = self
            .srgb_transform_cache
            .get_or_init(|| self.srgb_transform())
        else {
            return color;
        };
        let source = [color[0], color[1], color[2]];
        let mut converted = [0.0f32; 3];
        let width = if *gray { 1 } else { 3 };
        if transform
            .transform(&source, &mut converted[..width])
            .is_err()
        {
            return color;
        }
        match gray {
            true => [converted[0], converted[0], converted[0], color[3]],
            false => [converted[0], converted[1], converted[2], color[3]],
        }
    }

    fn srgb_transform(&self) -> Option<(Arc<TransformF32Executor>, bool)> {
        let profile = self.profile.as_ref().filter(|profile| !profile.is_srgb())?;
        let target = profile.parsed();
        let gray = target.color_space == DataColorSpace::Gray;
        let layout = match gray {
            true => Layout::Gray,
            false => Layout::Rgb,
        };
        let transform = Profile::srgb()
            .parsed()
            .create_transform_f32(Layout::Rgb, target, layout, TransformOptions::default())
            .ok()?;
        Some((transform, gray))
    }
}

impl std::fmt::Debug for Space {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Space")
            .field("profile", &self.profile)
            .field("transfer", &self.transfer)
            .finish_non_exhaustive()
    }
}

pub(crate) fn curves_of(profile: &ColorProfile) -> Option<[ToneReprCurve; 3]> {
    match profile.color_space {
        DataColorSpace::Gray => {
            let gray = profile.gray_trc.clone()?;
            Some([gray.clone(), gray.clone(), gray])
        }
        DataColorSpace::Rgb if profile.is_matrix_shaper() => Some([
            profile.red_trc.clone()?,
            profile.green_trc.clone()?,
            profile.blue_trc.clone()?,
        ]),
        _ => None,
    }
}

fn is_identity(curve: &ToneReprCurve) -> bool {
    match curve {
        ToneReprCurve::Lut(lut) => lut.is_empty() || (lut.len() == 1 && lut[0] == 0x0100),
        ToneReprCurve::Parametric(parameters) => {
            parameters.len() == 1 && (parameters[0] - 1.0).abs() < 1e-6
        }
    }
}

pub(crate) fn tables_of(curves: &[ToneReprCurve; 3]) -> Option<Vec<f32>> {
    let mut tables = vec![0.0f32; 6 * TABLE_SIZE];
    let last = (TABLE_SIZE - 1) as f32;
    for (channel, curve) in curves.iter().enumerate() {
        let forward = curve.make_linear_evaluator().ok()?;
        let backward = curve.make_gamma_evaluator().ok()?;
        for index in 0..TABLE_SIZE {
            let position = index as f32 / last;
            tables[channel * TABLE_SIZE + index] = forward.evaluate_value(position);
            tables[(3 + channel) * TABLE_SIZE + index] =
                backward.evaluate_value(position * position);
        }
    }
    Some(tables)
}

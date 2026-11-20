use moxcms::{ColorProfile, DataColorSpace, Layout, RenderingIntent, TransformOptions};
use rikka_gpu::ComputeBuffer;

use crate::color::profile::Profile;
use crate::color::space::{TABLE_SIZE, curves_of, tables_of};
use crate::error::{Error, Result};
use crate::work::Work;

const CUBE: usize = 33;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Intent {
    Perceptual,
    #[default]
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}

impl Intent {
    pub const ALL: [Intent; 4] = [
        Intent::Perceptual,
        Intent::RelativeColorimetric,
        Intent::Saturation,
        Intent::AbsoluteColorimetric,
    ];

    fn moxcms(self) -> RenderingIntent {
        match self {
            Intent::Perceptual => RenderingIntent::Perceptual,
            Intent::RelativeColorimetric => RenderingIntent::RelativeColorimetric,
            Intent::Saturation => RenderingIntent::Saturation,
            Intent::AbsoluteColorimetric => RenderingIntent::AbsoluteColorimetric,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConversionKind {
    Matrix = 0,
    Cube = 1,
    Identity = 2,
}

#[derive(Debug)]
pub(crate) struct Conversion {
    pub(crate) kind: ConversionKind,
    pub(crate) data: Vec<f32>,
    pub(crate) cube: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedConversion {
    pub(crate) kind: ConversionKind,
    pub(crate) cube: u32,
    pub(crate) data: ComputeBuffer,
}

impl Conversion {
    pub(crate) fn prepare(&self, work: &Work) -> Result<PreparedConversion> {
        Ok(PreparedConversion {
            kind: self.kind,
            cube: self.cube,
            data: work.constants(&self.data)?,
        })
    }
    pub(crate) fn new(
        from: Option<&Profile>,
        to: Option<&Profile>,
        intent: Intent,
    ) -> Result<Conversion> {
        let srgb = Profile::srgb();
        let from = from.unwrap_or(&srgb);
        let to = to.unwrap_or(&srgb);
        if from == to || (from.is_srgb() && to.is_srgb()) {
            return Ok(Conversion {
                kind: ConversionKind::Identity,
                data: Vec::new(),
                cube: 0,
            });
        }
        let source = from.parsed();
        let target = to.parsed();
        let use_matrix = source.is_matrix_shaper()
            && target.is_matrix_shaper()
            && intent != Intent::AbsoluteColorimetric;
        if use_matrix && let Some(conversion) = matrix(source, target) {
            return Ok(conversion);
        }
        cube(source, target, intent)
    }
}

fn matrix(source: &ColorProfile, target: &ColorProfile) -> Option<Conversion> {
    let source_tables = tables_of(&curves_of(source)?)?;
    let target_tables = tables_of(&curves_of(target)?)?;
    let mut data = Vec::with_capacity(6 * TABLE_SIZE + 12);
    data.extend_from_slice(&source_tables[..3 * TABLE_SIZE]);
    data.extend_from_slice(&target_tables[3 * TABLE_SIZE..]);
    let mixing = source.transform_matrix(target).v;
    for row in mixing {
        data.extend(row.iter().map(|&value| value as f32));
        data.push(0.0);
    }
    Some(Conversion {
        kind: ConversionKind::Matrix,
        data,
        cube: 0,
    })
}

fn cube(source: &ColorProfile, target: &ColorProfile, intent: Intent) -> Result<Conversion> {
    let gray_in = source.color_space == DataColorSpace::Gray;
    let gray_out = target.color_space == DataColorSpace::Gray;
    let options = TransformOptions {
        rendering_intent: intent.moxcms(),
        prefer_fixed_point: false,
        ..TransformOptions::default()
    };
    let layout = |gray| match gray {
        true => Layout::Gray,
        false => Layout::Rgb,
    };
    let transform = source
        .create_transform_f32(layout(gray_in), target, layout(gray_out), options)
        .map_err(|error| Error::Profile(error.to_string()))?;
    let last = (CUBE - 1) as f32;
    let mut inputs = Vec::with_capacity(CUBE * CUBE * CUBE * 3);
    for blue in 0..CUBE {
        for green in 0..CUBE {
            for red in 0..CUBE {
                let rgb = [red as f32 / last, green as f32 / last, blue as f32 / last];
                match gray_in {
                    true => inputs.push((rgb[0] + rgb[1] + rgb[2]) / 3.0),
                    false => inputs.extend_from_slice(&rgb),
                }
            }
        }
    }
    let width = if gray_out { 1 } else { 3 };
    let count = CUBE * CUBE * CUBE;
    let mut outputs = vec![0.0f32; count * width];
    transform
        .transform(&inputs, &mut outputs)
        .map_err(|error| Error::Profile(error.to_string()))?;
    let mut data = vec![0.0f32; 6 * TABLE_SIZE + 12];
    data.reserve(count * 3);
    for index in 0..count {
        match gray_out {
            true => data.extend_from_slice(&[outputs[index]; 3]),
            false => data.extend_from_slice(&outputs[index * 3..index * 3 + 3]),
        }
    }
    Ok(Conversion {
        kind: ConversionKind::Cube,
        data,
        cube: CUBE as u32,
    })
}

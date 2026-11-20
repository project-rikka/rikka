use std::sync::Arc;

use moxcms::{
    ColorProfile, DataColorSpace, Layout, RenderingIntent, TransformF32Executor, TransformOptions,
};

use crate::color::convert::{
    cmyk_to_rgb, lab_decoded, lab_encoded, lab_to_rgb, rgb_to_cmyk, rgb_to_gray, rgb_to_lab,
};
use crate::document::{ColorMode, Document};
use crate::pixels::Depth;
use crate::pixels::profile_issue::ProfileIssue;

#[derive(Clone)]
pub struct ColorConverter {
    mode: ColorMode,
    depth: Depth,
    palette: Option<Vec<[u8; 3]>>,
    transforms: Option<CmykTransforms>,
    profile_issue: Option<ProfileIssue>,
}

#[derive(Clone)]
struct CmykTransforms {
    to_rgb: Arc<TransformF32Executor>,
    from_rgb: Arc<TransformF32Executor>,
}

impl std::fmt::Debug for ColorConverter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorConverter")
            .field("mode", &self.mode)
            .field("depth", &self.depth)
            .field("managed", &self.transforms.is_some())
            .field("profile_issue", &self.profile_issue)
            .finish()
    }
}

impl ColorConverter {
    pub fn new(document: &Document) -> Self {
        let (transforms, profile_issue) =
            match (document.color_mode, document.resources.icc_profile()) {
                (ColorMode::Cmyk, Some(icc)) => match cmyk_transforms(icc) {
                    Ok(transforms) => (Some(transforms), None),
                    Err(issue) => (None, Some(issue)),
                },
                _ => (None, None),
            };
        Self {
            mode: document.color_mode,
            depth: document.depth,
            palette: document.color_mode_data.palette().map(<[_]>::to_vec),
            transforms,
            profile_issue,
        }
    }

    pub fn mode(&self) -> ColorMode {
        self.mode
    }

    pub fn is_managed(&self) -> bool {
        self.transforms.is_some()
    }

    pub fn profile_issue(&self) -> Option<&ProfileIssue> {
        self.profile_issue.as_ref()
    }

    pub fn is_linear(&self) -> bool {
        self.depth == Depth::ThirtyTwo
    }

    pub fn channels(&self) -> usize {
        self.mode.color_channels()
    }

    pub fn to_rgb(&self, channels: &[Vec<f32>]) -> Vec<[f32; 3]> {
        let count = channels.first().map_or(0, Vec::len);
        match self.mode {
            ColorMode::Rgb => (0..count)
                .map(|index| [channels[0][index], channels[1][index], channels[2][index]])
                .collect(),
            ColorMode::Indexed => {
                let palette = self.palette.as_deref().unwrap_or(&[]);
                channels[0]
                    .iter()
                    .map(|&value| {
                        let entry = palette
                            .get((value * 255.0).round() as usize)
                            .copied()
                            .unwrap_or([0; 3]);
                        entry.map(|component| f32::from(component) / 255.0)
                    })
                    .collect()
            }
            ColorMode::Cmyk => {
                let mut ink = Vec::with_capacity(count * 4);
                for index in 0..count {
                    for channel in channels.iter().take(4) {
                        ink.push(1.0 - channel[index]);
                    }
                }
                if let Some(transforms) = &self.transforms {
                    let mut rgb = vec![0.0f32; count * 3];
                    transforms
                        .to_rgb
                        .transform(&ink, &mut rgb)
                        .expect("buffers match the CMYK to RGB transform layout");
                    return rgb
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .map(|pixel| pixel.map(|v| v.clamp(0.0, 1.0)))
                        .collect();
                }
                ink.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|pixel| {
                        cmyk_to_rgb(
                            f64::from(pixel[0]),
                            f64::from(pixel[1]),
                            f64::from(pixel[2]),
                            f64::from(pixel[3]),
                        )
                        .map(|v| v as f32)
                    })
                    .collect()
            }
            ColorMode::Lab => (0..count)
                .map(|index| {
                    let [l, a, b] = lab_decoded(
                        [channels[0][index], channels[1][index], channels[2][index]],
                        self.depth,
                    );
                    lab_to_rgb(l, a, b).map(|v| v as f32)
                })
                .collect(),
            ColorMode::Bitmap
            | ColorMode::Grayscale
            | ColorMode::Duotone
            | ColorMode::Multichannel => channels[0].iter().map(|&value| [value; 3]).collect(),
        }
    }

    pub fn from_rgb(&self, rgb: &[[f32; 3]]) -> Vec<Vec<f32>> {
        match self.mode {
            ColorMode::Rgb => (0..3)
                .map(|channel| rgb.iter().map(|pixel| pixel[channel]).collect())
                .collect(),
            ColorMode::Cmyk => {
                let flat: Vec<f32> = rgb.iter().flatten().copied().collect();
                let mut ink = vec![0.0f32; rgb.len() * 4];
                match &self.transforms {
                    Some(transforms) => transforms
                        .from_rgb
                        .transform(&flat, &mut ink)
                        .expect("buffers match the RGB to CMYK transform layout"),
                    None => {
                        for (pixel, out) in rgb.iter().zip(ink.as_chunks_mut::<4>().0.iter_mut()) {
                            let values = rgb_to_cmyk(pixel.map(f64::from));
                            for (slot, value) in out.iter_mut().zip(values) {
                                *slot = value as f32;
                            }
                        }
                    }
                }
                (0..4)
                    .map(|channel| {
                        ink.as_chunks::<4>()
                            .0
                            .iter()
                            .map(|pixel| 1.0 - pixel[channel].clamp(0.0, 1.0))
                            .collect()
                    })
                    .collect()
            }
            ColorMode::Lab => {
                let encoded: Vec<[f32; 3]> = rgb
                    .iter()
                    .map(|pixel| self.encode_lab(rgb_to_lab(pixel.map(f64::from))))
                    .collect();
                (0..3)
                    .map(|channel| encoded.iter().map(|pixel| pixel[channel]).collect())
                    .collect()
            }
            _ => vec![rgb.iter().map(|pixel| rgb_to_gray(*pixel)).collect()],
        }
    }

    fn encode_lab(&self, lab: [f64; 3]) -> [f32; 3] {
        let [l, a, b] = lab_encoded(lab, self.depth);
        [l as f32, a.clamp(0.0, 1.0) as f32, b.clamp(0.0, 1.0) as f32]
    }
}

fn cmyk_transforms(icc: &[u8]) -> Result<CmykTransforms, ProfileIssue> {
    let profile = ColorProfile::new_from_slice(icc)
        .map_err(|error| ProfileIssue::Unreadable(error.to_string()))?;
    if profile.color_space != DataColorSpace::Cmyk {
        return Err(ProfileIssue::NotCmyk);
    }
    let options = TransformOptions {
        rendering_intent: RenderingIntent::RelativeColorimetric,
        prefer_fixed_point: false,
        ..TransformOptions::default()
    };
    let srgb = ColorProfile::new_srgb();
    let transform = |result: Result<Arc<TransformF32Executor>, _>| {
        result.map_err(|error: moxcms::CmsError| ProfileIssue::Transform(error.to_string()))
    };
    Ok(CmykTransforms {
        to_rgb: transform(profile.create_transform_f32(Layout::Rgba, &srgb, Layout::Rgb, options))?,
        from_rgb: transform(srgb.create_transform_f32(
            Layout::Rgb,
            &profile,
            Layout::Rgba,
            options,
        ))?,
    })
}

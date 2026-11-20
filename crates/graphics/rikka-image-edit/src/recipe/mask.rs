use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use rikka_gpu::ImageFormat;

use crate::error::{Error, Result};
use crate::marks::{FillRule, Path, path_reach, rasterize_path};
use crate::ops::Reach;
use crate::ops::passes::{CopyPush, Kernel1d, gaussian, gaussian_reach, resampled};
use crate::picture::{Depth, Picture};
use crate::plane::Plane;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const RECTANGLE: u32 = 0;
const ELLIPSE: u32 = 1;
const COPY: u32 = 2;
const LUMA: u32 = 3;
const ALPHA: u32 = 4;

const MAX_FEATHER: f32 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub y: u32,
    pub from: u32,
    pub to: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Channel {
    Luma,
    Alpha,
}

#[derive(Clone, Debug, PartialEq)]
enum Shape {
    Rectangle([f32; 4]),
    Ellipse([f32; 4]),
    Coverage {
        width: u32,
        height: u32,
        values: Arc<[u8]>,
    },
    Path {
        path: Path,
        rule: FillRule,
    },
    Matte {
        picture: Picture,
        channel: Channel,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    shape: Shape,
    feather: f32,
    inverted: bool,
    scale: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaskPush {
    head: PushHeader,
    area: [f32; 4],
    invert: f32,
    pad: [f32; 3],
}

impl Mask {
    #[must_use]
    pub fn luma(picture: Picture) -> Mask {
        Mask::shaped(Shape::Matte {
            picture,
            channel: Channel::Luma,
        })
    }

    #[must_use]
    pub fn alpha(picture: Picture) -> Mask {
        Mask::shaped(Shape::Matte {
            picture,
            channel: Channel::Alpha,
        })
    }

    #[must_use]
    pub fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Mask {
        Mask::shaped(Shape::Rectangle([x, y, width.max(0.0), height.max(0.0)]))
    }

    #[must_use]
    pub fn ellipse(x: f32, y: f32, width: f32, height: f32) -> Mask {
        Mask::shaped(Shape::Ellipse([x, y, width.max(0.0), height.max(0.0)]))
    }

    #[must_use]
    pub fn path(path: Path, rule: FillRule) -> Mask {
        Mask::shaped(Shape::Path { path, rule })
    }

    pub fn spans(width: u32, height: u32, spans: impl IntoIterator<Item = Span>) -> Result<Mask> {
        let mut values = vec![0u8; pixel_count(width, height)?];
        for span in spans {
            if span.y >= height {
                continue;
            }
            let row = span.y as usize * width as usize;
            let (from, to) = (span.from.min(width) as usize, span.to.min(width) as usize);
            if from < to {
                values[row + from..row + to].fill(u8::MAX);
            }
        }
        Ok(Mask::shaped(Shape::Coverage {
            width,
            height,
            values: values.into(),
        }))
    }

    pub fn coverage(width: u32, height: u32, values: impl Into<Arc<[u8]>>) -> Result<Mask> {
        let values = values.into();
        if values.len() != pixel_count(width, height)? {
            return Err(Error::Invalid(format!(
                "{width}×{height} mask has {} values, expected {}",
                values.len(),
                width as usize * height as usize
            )));
        }
        Ok(Mask::shaped(Shape::Coverage {
            width,
            height,
            values,
        }))
    }

    fn shaped(shape: Shape) -> Mask {
        Mask {
            shape,
            feather: 0.0,
            inverted: false,
            scale: 1.0,
        }
    }

    #[must_use]
    pub fn feathered(mut self, radius: f32) -> Mask {
        self.feather = radius.clamp(0.0, MAX_FEATHER);
        self
    }

    #[must_use]
    pub fn inverted(mut self) -> Mask {
        self.inverted = !self.inverted;
        self
    }

    #[must_use]
    pub fn scaled(&self, factor: f32) -> Mask {
        Mask {
            scale: self.scale * factor,
            ..self.clone()
        }
    }

    pub(crate) fn reach(&self, canvas: [u32; 2]) -> Reach {
        let Some(halo) = gaussian_reach(self.feather * self.scale / 2.0) else {
            return Reach::Global;
        };
        if self.inverted {
            return Reach::Everywhere { halo };
        }
        let shaped = match &self.shape {
            Shape::Rectangle(area) | Shape::Ellipse(area) => {
                let [x, y, width, height] = area.map(|value| value * self.scale);
                let left = (x.floor() - 1.0).max(0.0);
                let top = (y.floor() - 1.0).max(0.0);
                let right = ((x + width).ceil() + 1.0).min(canvas[0] as f32);
                let bottom = ((y + height).ceil() + 1.0).min(canvas[1] as f32);
                match right > left && bottom > top {
                    true => Reach::bounded([
                        left as u32,
                        top as u32,
                        (right - left) as u32,
                        (bottom - top) as u32,
                    ]),
                    false => Reach::Nowhere,
                }
            }
            Shape::Coverage {
                width,
                height,
                values,
            } => match [*width, *height] == canvas {
                true => coverage_reach(*width, values),
                false => Reach::Global,
            },
            Shape::Path { path, rule } => path_reach(path, *rule, self.scale, canvas),
            Shape::Matte { .. } => Reach::Global,
        };
        match shaped {
            Reach::Within { area, .. } => Reach::Within {
                area: grown(area, halo, canvas),
                halo,
            },
            reach => reach,
        }
    }

    pub(crate) fn develop(
        &self,
        work: &mut Work,
        canvas: [u32; 2],
        template: &Plane,
        depth: Depth,
    ) -> Result<Plane> {
        let format = depth.mask(work.imaging().formats());
        let invert = if self.inverted { 1.0 } else { 0.0 };
        let shaped = match &self.shape {
            Shape::Rectangle(area) | Shape::Ellipse(area) => {
                let mode = match self.shape {
                    Shape::Rectangle(_) => RECTANGLE,
                    _ => ELLIPSE,
                };
                let out = work.plane_like(template, format)?;
                let push = MaskPush {
                    head: PushHeader::new(&out, &out).mode(mode),
                    area: area.map(|value| value * self.scale),
                    invert: 0.0,
                    pad: [0.0; 3],
                };
                work.pass(Shader::Mask, push, Io::new(&out, &out), 1.0)?;
                out
            }
            Shape::Coverage {
                width,
                height,
                values,
            } => {
                let upload_format = work.imaging().formats().shallow_mask;
                let raw = work.plane([*width, *height], upload_format)?;
                let bytes: Vec<u8> = match upload_format {
                    ImageFormat::R8 => values.to_vec(),
                    _ => values
                        .iter()
                        .flat_map(|&value| (f32::from(value) / 255.0).to_ne_bytes())
                        .collect(),
                };
                crate::picture::upload(work, &raw, &bytes);
                match (raw.extent() == canvas, template.extent() == canvas) {
                    (true, true) => raw,
                    (true, false) => cropped(work, &raw, template.extent())?,
                    (false, _) => resampled(work, &raw, canvas, Kernel1d::Box)?,
                }
            }
            Shape::Path { path, rule } => {
                let out = work.plane_like(template, format)?;
                rasterize_path(work, path, *rule, self.scale, &out)?;
                out
            }
            Shape::Matte { picture, channel } => {
                let source = &picture.frame().plane;
                let raw = work.plane(source.extent(), format)?;
                let mode = match channel {
                    Channel::Luma => LUMA,
                    Channel::Alpha => ALPHA,
                };
                let push = MaskPush {
                    head: PushHeader::new(source, &raw).mode(mode),
                    area: [0.0; 4],
                    invert: 0.0,
                    pad: [0.0; 3],
                };
                work.pass(Shader::Mask, push, Io::new(source, &raw), 1.0)?;
                match (raw.extent() == canvas, template.extent() == canvas) {
                    (true, true) => raw,
                    (true, false) => cropped(work, &raw, template.extent())?,
                    (false, _) => resampled(work, &raw, canvas, Kernel1d::Triangle)?,
                }
            }
        };
        let softened = gaussian(work, &shaped, self.feather * self.scale / 2.0)?;
        if !self.inverted && softened.format() == format {
            return Ok(softened);
        }
        let out = work.plane_like(&softened, format)?;
        let push = MaskPush {
            head: PushHeader::new(&softened, &out).mode(COPY),
            area: [0.0; 4],
            invert,
            pad: [0.0; 3],
        };
        work.pass(Shader::Mask, push, Io::new(&softened, &out), 1.0)?;
        Ok(out)
    }
}

fn cropped(work: &mut Work, raw: &Plane, extent: [u32; 2]) -> Result<Plane> {
    let out = work.plane(extent, raw.format())?;
    let push = CopyPush {
        head: PushHeader::new(raw, &out),
        from: work.window(),
        quarter: 0,
        mirror: 0,
        fill: [0.0; 4],
    };
    work.pass(Shader::Copy, push, Io::new(raw, &out), 1.0)?;
    Ok(out)
}

fn coverage_reach(width: u32, values: &[u8]) -> Reach {
    let width = width as usize;
    let mut bounds: Option<[usize; 4]> = None;
    for (row, line) in values.chunks_exact(width).enumerate() {
        let Some(first) = line.iter().position(|&value| value != 0) else {
            continue;
        };
        let last = line
            .iter()
            .rposition(|&value| value != 0)
            .expect("row has a non-zero value");
        bounds = Some(match bounds {
            None => [first, row, last + 1, row + 1],
            Some([left, top, right, _]) => [left.min(first), top, right.max(last + 1), row + 1],
        });
    }
    bounds.map_or(Reach::Nowhere, |[left, top, right, bottom]| {
        Reach::bounded([
            left as u32,
            top as u32,
            (right - left) as u32,
            (bottom - top) as u32,
        ])
    })
}

pub(crate) fn grown(area: [u32; 4], by: u32, canvas: [u32; 2]) -> [u32; 4] {
    let [x, y, width, height] = area;
    let left = x.saturating_sub(by);
    let top = y.saturating_sub(by);
    let right = (x + width).saturating_add(by).min(canvas[0]);
    let bottom = (y + height).saturating_add(by).min(canvas[1]);
    [left, top, right - left, bottom - top]
}

fn pixel_count(width: u32, height: u32) -> Result<usize> {
    match width > 0 && height > 0 {
        true => Ok(width as usize * height as usize),
        false => Err(Error::Invalid(format!("{width}×{height} mask is empty"))),
    }
}

crate::work::push_header!(MaskPush);

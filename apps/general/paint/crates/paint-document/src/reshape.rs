use rikka_image_edit::{
    Anchor, CanvasSize, Flip, Imaging, QuarterTurn, Recipe, Resampling, Resize, Transform,
};
use rikka_psd::{Affine, Axis, Geometry, ResampleOptions};
use rikka_types::Color;

use crate::area::Area;
use crate::error::{Error, Result};
use crate::region::Region;
use crate::resampler::{GpuResampler, matrix};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reshape {
    Resize {
        width: u32,
        height: u32,
        resampling: Resampling,
    },
    CanvasSize {
        width: u32,
        height: u32,
        anchor: Anchor,
        fill: Color,
    },
    Turn(QuarterTurn),
    Flip(Flip),
    Rotate {
        degrees: f32,
        resampling: Resampling,
        fill: Color,
    },
}

impl Reshape {
    pub(crate) fn recipe(self, from: (u32, u32)) -> Recipe {
        match self {
            Reshape::Resize {
                width,
                height,
                resampling,
            } => Recipe::new().then(Resize::new(width, height).resampling(resampling)),
            Reshape::CanvasSize {
                width,
                height,
                anchor,
                fill,
            } => Recipe::new().then(CanvasSize::new(width, height).anchor(anchor).fill(fill)),
            Reshape::Turn(turn) => Recipe::new().then(turn),
            Reshape::Flip(flip) => Recipe::new().then(flip),
            Reshape::Rotate {
                degrees,
                resampling,
                fill,
            } => {
                let turned = Geometry::rotate(from, f64::from(degrees));
                Recipe::new().then(
                    Transform::new(matrix(turned.map), turned.width, turned.height)
                        .resampling(resampling)
                        .fill(fill),
                )
            }
        }
    }

    pub(crate) fn map(self, from: (u32, u32)) -> Affine {
        let (width, height) = (f64::from(from.0), f64::from(from.1));
        match self {
            Reshape::Resize {
                width: to_width,
                height: to_height,
                ..
            } => Geometry::resize(from, to_width, to_height).map,
            Reshape::CanvasSize {
                width: to_width,
                height: to_height,
                anchor,
                ..
            } => {
                let (x, y) = anchor.offset((to_width, to_height), from);
                Affine::translation(x as f64, y as f64)
            }
            Reshape::Turn(QuarterTurn::Clockwise) => {
                Affine::from_array([0.0, 1.0, -1.0, 0.0, height, 0.0])
            }
            Reshape::Turn(QuarterTurn::CounterClockwise) => {
                Affine::from_array([0.0, -1.0, 1.0, 0.0, 0.0, width])
            }
            Reshape::Turn(QuarterTurn::Half) => {
                Affine::from_array([-1.0, 0.0, 0.0, -1.0, width, height])
            }
            Reshape::Flip(Flip::Horizontal) => {
                Affine::from_array([-1.0, 0.0, 0.0, 1.0, width, 0.0])
            }
            Reshape::Flip(Flip::Vertical) => Affine::from_array([1.0, 0.0, 0.0, -1.0, 0.0, height]),
            Reshape::Rotate { degrees, .. } => Geometry::rotate(from, f64::from(degrees)).map,
        }
    }

    pub(crate) fn scales(self) -> bool {
        matches!(self, Reshape::Resize { .. })
    }

    pub(crate) fn uprights_text(self) -> bool {
        matches!(self, Reshape::Resize { .. } | Reshape::CanvasSize { .. })
    }

    pub(crate) fn cleared(self) -> Reshape {
        match self {
            Reshape::CanvasSize {
                width,
                height,
                anchor,
                ..
            } => Reshape::CanvasSize {
                width,
                height,
                anchor,
                fill: Color::TRANSPARENT,
            },
            Reshape::Rotate {
                degrees,
                resampling,
                ..
            } => Reshape::Rotate {
                degrees,
                resampling,
                fill: Color::TRANSPARENT,
            },
            other => other,
        }
    }

    pub(crate) fn changes_nothing(self, width: u32, height: u32) -> bool {
        match self {
            Reshape::Resize {
                width: to_width,
                height: to_height,
                ..
            }
            | Reshape::CanvasSize {
                width: to_width,
                height: to_height,
                ..
            } => (to_width, to_height) == (width, height),
            Reshape::Rotate { degrees, .. } => degrees.rem_euclid(360.0) == 0.0,
            Reshape::Turn(_) | Reshape::Flip(_) => false,
        }
    }

    pub(crate) fn selection(self, selection: &Region, width: u32, height: u32) -> Region {
        let canvas = Area::sized(width, height);
        match self {
            Reshape::Turn(turn) => selection.turned(turn, canvas),
            Reshape::Flip(flip) => selection.flipped(flip, canvas),
            Reshape::CanvasSize {
                width: to_width,
                height: to_height,
                anchor,
                ..
            } => {
                let (x, y) = anchor.offset((to_width, to_height), (width, height));
                selection.translated(x, y, Area::sized(to_width, to_height))
            }
            Reshape::Resize { .. } | Reshape::Rotate { .. } => Region::default(),
        }
    }
}

pub(crate) fn photoshop(
    imaging: &Imaging,
    document: &rikka_psd::Document,
    reshape: Reshape,
) -> Result<rikka_psd::Document> {
    let from = (document.width, document.height);
    let mut reshaped = document.clone();
    match reshape {
        Reshape::Turn(turn) => reshaped.rotate_quarter(quarter(turn))?,
        Reshape::Flip(flip) => reshaped.flip(axis(flip))?,
        Reshape::CanvasSize {
            width,
            height,
            anchor,
            fill,
        } => {
            let (x, y) = anchor.offset((width, height), from);
            let shift = |by: i64| i32::try_from(-by).map_err(|_| Error::TooLarge { width, height });
            let canvas = rikka_psd::Rect::from_size(shift(x)?, shift(y)?, width, height);
            reshaped.resize_canvas(canvas, background(fill))?;
        }
        Reshape::Resize {
            width,
            height,
            resampling,
        } => resample(
            imaging,
            &mut reshaped,
            Geometry::resize(from, width, height),
            resampling,
            Color::WHITE,
        )?,
        Reshape::Rotate {
            degrees,
            resampling,
            fill,
        } => resample(
            imaging,
            &mut reshaped,
            Geometry::rotate(from, f64::from(degrees)),
            resampling,
            fill,
        )?,
    }
    Ok(reshaped)
}

fn resample(
    imaging: &Imaging,
    document: &mut rikka_psd::Document,
    geometry: Geometry,
    resampling: Resampling,
    fill: Color,
) -> Result<()> {
    let mut resampler = GpuResampler::new(imaging, resampling);
    let options = ResampleOptions {
        background: background(fill),
        ..ResampleOptions::default()
    };
    document
        .resample(geometry, &options, &mut resampler)
        .map_err(|error| resampler.failed(error))
}

fn background(fill: Color) -> rikka_psd::Color {
    let fill = match fill.a > 0.0 {
        true => fill,
        false => Color::WHITE,
    };
    rikka_psd::Color::Rgb {
        r: f64::from(fill.r),
        g: f64::from(fill.g),
        b: f64::from(fill.b),
    }
}

fn quarter(turn: QuarterTurn) -> rikka_psd::QuarterTurn {
    match turn {
        QuarterTurn::Clockwise => rikka_psd::QuarterTurn::Clockwise,
        QuarterTurn::Half => rikka_psd::QuarterTurn::Half,
        QuarterTurn::CounterClockwise => rikka_psd::QuarterTurn::CounterClockwise,
    }
}

fn axis(flip: Flip) -> Axis {
    match flip {
        Flip::Horizontal => Axis::Horizontal,
        Flip::Vertical => Axis::Vertical,
    }
}

pub(crate) fn resamples(reshape: Reshape) -> bool {
    matches!(reshape, Reshape::Resize { .. } | Reshape::Rotate { .. })
}

use rikka_image::Image;
use rikka_image_edit::{Imaging, Picture, Profile, Recipe, Resampling, Resize, Transform};
use rikka_psd::{Affine, CpuResampler, Edge, Kernel, Plane};
use rikka_types::Color;

use crate::develop::{floats_of, stored_at};
use crate::error::{Error, Result};

const ALIGNED: f64 = 1e-9;

pub(crate) struct GpuResampler<'a> {
    imaging: &'a Imaging,
    resampling: Resampling,
    failure: Option<Error>,
}

impl<'a> GpuResampler<'a> {
    pub(crate) fn new(imaging: &'a Imaging, resampling: Resampling) -> GpuResampler<'a> {
        GpuResampler {
            imaging,
            resampling,
            failure: None,
        }
    }

    pub(crate) fn failed(&mut self, error: rikka_psd::Error) -> Error {
        self.failure.take().unwrap_or_else(|| Error::Psd(error))
    }

    fn on_gpu(&self, job: &rikka_psd::Resampling<'_>) -> Result<Plane> {
        let source = self.source(job)?;
        let step = match fills_extent(job) {
            true => {
                Recipe::new().then(Resize::new(job.width, job.height).resampling(self.resampling))
            }
            false => Recipe::new().then(
                Transform::new(matrix(inverse(job.map)?), job.width, job.height)
                    .resampling(self.warp())
                    .fill(edge_fill(job)),
            ),
        };
        let resampled = source.develop(&step)?;
        let values: Vec<f32> = floats_of(&resampled.to_image()?)
            .into_iter()
            .map(|[value, ..]| value)
            .collect();
        Plane::from_normalized(job.width, job.height, job.source.depth(), &values)
            .map_err(Error::from)
    }

    fn source(&self, job: &rikka_psd::Resampling<'_>) -> Result<Picture> {
        let values = job.source.to_normalized();
        let weights = job.weight.map(Plane::to_normalized);
        let pixels: Vec<f32> = values
            .iter()
            .enumerate()
            .flat_map(|(index, &value)| {
                let weight = weights.as_ref().map_or(1.0, |weights| weights[index]);
                [value, value, value, weight]
            })
            .collect();
        let image = Image::from_rgba32f(job.source.width(), job.source.height(), pixels)?;
        let image = stored_at(image, job.source.depth());
        Ok(self
            .imaging
            .picture(&image)?
            .with_profile(Some(Profile::linear_srgb())))
    }

    fn warp(&self) -> Resampling {
        match self.resampling {
            Resampling::Nearest | Resampling::Bilinear => self.resampling,
            Resampling::Bicubic | Resampling::BicubicSmoother | Resampling::Lanczos => {
                Resampling::Bicubic
            }
        }
    }
}

impl rikka_psd::Resampler for GpuResampler<'_> {
    fn resample(&mut self, job: &rikka_psd::Resampling<'_>) -> rikka_psd::Result<Plane> {
        if job.discrete || job.source.depth() == rikka_psd::Depth::One {
            return CpuResampler::new(Kernel::Nearest).resample(job);
        }
        self.on_gpu(job).map_err(|error| {
            self.failure.get_or_insert(error);
            rikka_psd::Error::Planes("the graphics card could not resample a channel")
        })
    }
}

fn fills_extent(job: &rikka_psd::Resampling<'_>) -> bool {
    let map = job.map;
    let scale_x = f64::from(job.source.width()) / f64::from(job.width);
    let scale_y = f64::from(job.source.height()) / f64::from(job.height);
    map.is_axis_aligned()
        && map.tx.abs() < ALIGNED
        && map.ty.abs() < ALIGNED
        && (map.xx - scale_x).abs() < ALIGNED * scale_x.max(1.0)
        && (map.yy - scale_y).abs() < ALIGNED * scale_y.max(1.0)
}

fn edge_fill(job: &rikka_psd::Resampling<'_>) -> Color {
    match (job.edge, job.weight) {
        (Edge::Constant(value), None) => Color::rgba(value, value, value, 1.0),
        (Edge::Constant(value), Some(_)) => Color::rgba(value, value, value, 0.0),
        (Edge::Clamp, _) => Color::TRANSPARENT,
    }
}

fn inverse(map: Affine) -> Result<Affine> {
    map.inverse().ok_or_else(|| Error::SingularMap)
}

pub(crate) fn matrix(map: Affine) -> [[f64; 3]; 3] {
    [
        [map.xx, map.yx, map.tx],
        [map.xy, map.yy, map.ty],
        [0.0, 0.0, 1.0],
    ]
}

use glam::Vec3;

use crate::error::{Error, Result};
use crate::fluid::gpu::GpuFluidMaterial;

const CLOUD_RANGE: f32 = 4000.0;
const THINNEST: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidMaterial {
    pub color: Vec3,
    pub depth: f32,
    pub cloudiness: f32,
    pub roughness: f32,
    pub ior: f32,
}

impl Default for FluidMaterial {
    fn default() -> Self {
        Self {
            color: Vec3::new(0.72, 0.93, 0.97),
            depth: 1.0,
            cloudiness: 0.0,
            roughness: 0.02,
            ior: 1.333,
        }
    }
}

impl FluidMaterial {
    pub(crate) fn check(&self) -> Result<()> {
        let finite = self.color.is_finite()
            && [self.depth, self.cloudiness, self.roughness, self.ior]
                .iter()
                .all(|value| value.is_finite());
        if !finite {
            return Err(Error::Invalid(format!(
                "fluid material contains NaN: {self:?}"
            )));
        }
        Ok(())
    }

    pub(crate) fn to_gpu(self) -> GpuFluidMaterial {
        let color = self.color.clamp(Vec3::splat(THINNEST), Vec3::ONE);
        let absorption = -color.map(f32::ln) / self.depth.max(THINNEST);
        let cloudiness = self.cloudiness.clamp(0.0, 1.0);
        let scattering = CLOUD_RANGE.powf(cloudiness) - 1.0;
        GpuFluidMaterial {
            absorption: absorption.to_array(),
            scattering,
            albedo: color.to_array(),
            roughness: self.roughness.clamp(0.0, 1.0),
            ior: self.ior.clamp(1.0, 3.0),
            extinction: absorption.element_sum() / 3.0 + scattering,
            _pad: [0.0; 2],
        }
    }
}

use crate::error::{Error, Result};
use crate::fluid::gpu::{PARTICLE_BYTES, SPRAY_BYTES};
use crate::fluid::material::FluidMaterial;
use rikka_gpu::compute::{ComputeBuffer, Ticket};

#[derive(Clone, Debug)]
#[must_use]
pub struct FluidLayer {
    pub(crate) particles: ComputeBuffer,
    pub(crate) count: u32,
    pub(crate) radius: f32,
    pub(crate) ready: Ticket,
    pub(crate) spray: Option<(ComputeBuffer, u32)>,
    pub(crate) materials: Vec<FluidMaterial>,
}

impl FluidLayer {
    pub fn new(particles: &ComputeBuffer, count: u32, radius: f32, ready: &Ticket) -> Self {
        Self {
            particles: particles.clone(),
            count,
            radius,
            ready: ready.clone(),
            spray: None,
            materials: Vec::new(),
        }
    }

    pub fn spray(mut self, spray: &ComputeBuffer, count: u32) -> Self {
        self.spray = Some((spray.clone(), count));
        self
    }

    pub fn material(mut self, material: FluidMaterial) -> Self {
        self.materials.push(material);
        self
    }

    pub fn materials(mut self, materials: impl IntoIterator<Item = FluidMaterial>) -> Self {
        self.materials.extend(materials);
        self
    }

    pub(crate) fn spray_count(&self) -> u32 {
        self.spray.as_ref().map_or(0, |(_, count)| *count)
    }

    pub(crate) fn check(&self) -> Result<()> {
        if !(self.radius.is_finite() && self.radius > 0.0) {
            return Err(Error::Invalid(format!(
                "fluid particle radius must be positive, got {}",
                self.radius
            )));
        }
        holds(&self.particles, self.count, PARTICLE_BYTES, "particles")?;
        if let Some((spray, count)) = &self.spray {
            holds(spray, *count, SPRAY_BYTES, "spray particles")?;
        }
        for buffer in self.buffers() {
            if !buffer.belongs_to(&self.ready.shared) {
                return Err(Error::Invalid(
                    "fluid layer buffers and ticket belong to different compute contexts".into(),
                ));
            }
        }
        self.materials.iter().try_for_each(FluidMaterial::check)
    }

    pub(crate) fn buffers(&self) -> impl Iterator<Item = &ComputeBuffer> {
        std::iter::once(&self.particles).chain(self.spray.as_ref().map(|(buffer, _)| buffer))
    }
}

fn holds(buffer: &ComputeBuffer, count: u32, stride: u64, what: &str) -> Result<()> {
    let needed = u64::from(count) * stride;
    match needed <= buffer.size() {
        true => Ok(()),
        false => Err(Error::Invalid(format!(
            "{count} {what} need {needed} bytes but the buffer has {}",
            buffer.size()
        ))),
    }
}

use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::error::Result;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[repr(u32)]
pub enum Filtering {
    #[default]
    LinearRepeat = 0,
    LinearClamp = 1,
    NearestRepeat = 2,
    NearestClamp = 3,
    AnisotropicRepeat = 4,
}

impl Filtering {
    pub const COUNT: usize = 5;

    pub fn slot(self) -> u32 {
        self as u32
    }
}

pub const SLOT_SHADOW: u32 = Filtering::COUNT as u32;

pub const SLOT_DEPTH_REDUCE: u32 = SLOT_SHADOW + 1;

pub struct Samplers {
    device: Arc<RenderDevice>,
    samplers: Vec<vk::Sampler>,
}

impl Samplers {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Self> {
        let three_d = device.three_d();
        let anisotropic = device.filters_anisotropically();
        let max_anisotropy = device.limits().max_sampler_anisotropy.min(16.0);
        let mut samplers = Vec::with_capacity(Filtering::COUNT + 2);
        let mut built = Self {
            device,
            samplers: Vec::new(),
        };

        let plan = [
            (
                vk::Filter::LINEAR,
                vk::SamplerAddressMode::REPEAT,
                true,
                false,
            ),
            (
                vk::Filter::LINEAR,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                true,
                false,
            ),
            (
                vk::Filter::NEAREST,
                vk::SamplerAddressMode::REPEAT,
                false,
                false,
            ),
            (
                vk::Filter::NEAREST,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                false,
                false,
            ),
            (
                vk::Filter::LINEAR,
                vk::SamplerAddressMode::REPEAT,
                true,
                anisotropic,
            ),
        ];
        for (filter, address, mips, anisotropy) in plan {
            match built.create(filter, address, mips, anisotropy.then_some(max_anisotropy)) {
                Ok(sampler) => samplers.push(sampler),
                Err(err) => {
                    built.samplers = samplers;
                    return Err(err);
                }
            }
        }
        if three_d {
            for created in [built.create_shadow(), built.create_depth_reduce()] {
                match created {
                    Ok(sampler) => samplers.push(sampler),
                    Err(err) => {
                        built.samplers = samplers;
                        return Err(err);
                    }
                }
            }
        }
        built.samplers = samplers;
        Ok(built)
    }

    fn create(
        &self,
        filter: vk::Filter,
        address: vk::SamplerAddressMode,
        mips: bool,
        anisotropy: Option<f32>,
    ) -> Result<vk::Sampler> {
        let mut info = vk::SamplerCreateInfo::default()
            .mag_filter(filter)
            .min_filter(filter)
            .mipmap_mode(match filter {
                vk::Filter::LINEAR => vk::SamplerMipmapMode::LINEAR,
                _ => vk::SamplerMipmapMode::NEAREST,
            })
            .address_mode_u(address)
            .address_mode_v(address)
            .address_mode_w(address)
            .max_lod(match mips {
                true => vk::LOD_CLAMP_NONE,
                false => 0.0,
            });
        if let Some(max_anisotropy) = anisotropy {
            info = info.anisotropy_enable(true).max_anisotropy(max_anisotropy);
        }
        Ok(unsafe { self.device.handle().create_sampler(&info, None)? })
    }

    fn create_shadow(&self) -> Result<vk::Sampler> {
        let info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
            .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
            .compare_enable(true)
            .compare_op(vk::CompareOp::GREATER_OR_EQUAL);
        Ok(unsafe { self.device.handle().create_sampler(&info, None)? })
    }

    fn create_depth_reduce(&self) -> Result<vk::Sampler> {
        let mut reduction = vk::SamplerReductionModeCreateInfo::default()
            .reduction_mode(vk::SamplerReductionMode::MIN);
        let info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .min_lod(0.0)
            .max_lod(vk::LOD_CLAMP_NONE)
            .push_next(&mut reduction);
        Ok(unsafe { self.device.handle().create_sampler(&info, None)? })
    }

    pub fn linear_clamp(&self) -> vk::Sampler {
        self.samplers[Filtering::LinearClamp.slot() as usize]
    }

    pub fn nearest_clamp(&self) -> vk::Sampler {
        self.samplers[Filtering::NearestClamp.slot() as usize]
    }

    pub fn shadow(&self) -> vk::Sampler {
        self.samplers[SLOT_SHADOW as usize]
    }

    pub(crate) fn table(&self) -> &[vk::Sampler] {
        &self.samplers
    }

    pub fn depth_reduce(&self) -> vk::Sampler {
        self.samplers[SLOT_DEPTH_REDUCE as usize]
    }
}

impl Drop for Samplers {
    fn drop(&mut self) {
        unsafe {
            for &sampler in &self.samplers {
                self.device.handle().destroy_sampler(sampler, None);
            }
        }
    }
}

impl std::fmt::Debug for Samplers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Samplers").finish_non_exhaustive()
    }
}

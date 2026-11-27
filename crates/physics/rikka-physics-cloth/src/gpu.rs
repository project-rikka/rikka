use rikka_gpu::Compute;
use rikka_physics_gpu::NamedKernel;

use crate::error::Result;

const WIDE_CACHE: u32 = 3840;
const NARROW_CACHE: u32 = 2560;
const CACHED_BYTES: u32 = 12;
const SOLVER_SHARED_BYTES: u32 = 4 + 256 * 4;

rikka_physics_gpu::kernels! {
    pub(crate) struct Kernels from cloth_kernels {
        skin_body => SKIN_BODY_COMP,
        targets => TARGETS_COMP,
        normals => NORMALS_COMP,
        drape => DRAPE_COMP,
        obstacles => OBSTACLES_COMP,
        probe => PROBE_COMP,
        wraps => WRAPS_COMP,
        absorb => ABSORB_COMP,
        hash => HASH_COMP,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Variant {
    Narrow,
    Wide,
    Global,
}

pub(crate) struct Solvers {
    narrow: Option<NamedKernel>,
    wide: Option<NamedKernel>,
    global: NamedKernel,
}

impl Solvers {
    pub(crate) fn new(compute: &Compute) -> Result<Solvers> {
        let room = compute.workgroup_memory();
        let fits = |cache: u32| cache * CACHED_BYTES + SOLVER_SHARED_BYTES <= room;
        let wide = if fits(WIDE_CACHE) {
            Some(NamedKernel::load(
                compute,
                cloth_kernels::SOLVE_WIDE_COMP,
                "solve_wide",
            )?)
        } else {
            None
        };
        let narrow = if wide.is_none() && fits(NARROW_CACHE) {
            Some(NamedKernel::load(
                compute,
                cloth_kernels::SOLVE_NARROW_COMP,
                "solve_narrow",
            )?)
        } else {
            None
        };
        Ok(Solvers {
            narrow,
            wide,
            global: NamedKernel::load(compute, cloth_kernels::SOLVE_GLOBAL_COMP, "solve_global")?,
        })
    }

    pub(crate) fn variant(&self, particles: u32) -> Variant {
        if self.wide.is_some() && particles <= WIDE_CACHE {
            return Variant::Wide;
        }
        if self.narrow.is_some() && particles <= NARROW_CACHE {
            return Variant::Narrow;
        }
        Variant::Global
    }

    pub(crate) fn kernel(&self, variant: Variant) -> &NamedKernel {
        match variant {
            Variant::Narrow => self.narrow.as_ref().unwrap_or(&self.global),
            Variant::Wide => self.wide.as_ref().unwrap_or(&self.global),
            Variant::Global => &self.global,
        }
    }
}

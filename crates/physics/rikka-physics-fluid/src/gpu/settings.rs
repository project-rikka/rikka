use bytemuck::{Pod, Zeroable};

use crate::substance::{MAX_SUBSTANCES, SubstanceBlock};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct SettingsBlock {
    pub(crate) kernel: [f32; 4],
    pub(crate) solver: [f32; 4],
    pub(crate) body: [f32; 4],
    pub(crate) bounds_lo: [f32; 4],
    pub(crate) bounds_hi: [f32; 4],
    pub(crate) grid: [u32; 4],
    pub(crate) grid_cell: [f32; 4],
    pub(crate) tri_grid: [u32; 4],
    pub(crate) tri_cell: [f32; 4],
    pub(crate) look: [f32; 4],
    pub(crate) look_limits: [f32; 4],
    pub(crate) foam_potential: [f32; 4],
    pub(crate) foam_energy: [f32; 4],
    pub(crate) foam_motion: [f32; 4],
    pub(crate) foam_kinds: [f32; 4],
    pub(crate) boundary: [f32; 4],
    pub(crate) wall_shape: [f32; 4],
    pub(crate) closing: [f32; 4],
    pub(crate) reaction: [f32; 4],
    pub(crate) wall: [[f32; 4]; 32],
    pub(crate) substances: [SubstanceBlock; MAX_SUBSTANCES],
    pub(crate) surface: [[f32; 4]; 8],
}

pub(crate) const NARROWEST_GRID: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Grid {
    pub(crate) bits: [u32; 3],
}

impl Grid {
    pub(crate) fn fitted(self, cells: [f32; 3], max_bins: u32) -> Grid {
        let cap = max_bins.next_power_of_two().trailing_zeros();
        let most: u32 = self.bits.iter().sum::<u32>().min(cap);
        let mut bits = [NARROWEST_GRID; 3];
        let load = |bits: &[u32; 3], axis: usize| cells[axis] / (1u64 << bits[axis]) as f32;
        for _ in 3 * NARROWEST_GRID..most {
            let mut crowded = 0;
            for axis in [2, 1] {
                if load(&bits, axis) > load(&bits, crowded) {
                    crowded = axis;
                }
            }
            if load(&bits, crowded) <= 1.0 {
                break;
            }
            bits[crowded] += 1;
        }
        Grid { bits }
    }

    pub(crate) fn sized(bins: u32, minimum: u32) -> Grid {
        let total = bins.next_power_of_two().trailing_zeros().max(3 * minimum);
        let x = total.div_ceil(3).max(minimum);
        let z = (total - x).div_ceil(2).max(minimum);
        let y = total.saturating_sub(x + z).max(minimum);
        Grid { bits: [x, y, z] }
    }

    pub(crate) fn bins(self) -> u32 {
        1 << (self.bits[0] + self.bits[1] + self.bits[2])
    }

    pub(crate) fn block(self, enabled: bool) -> [u32; 4] {
        [
            self.bits[0],
            self.bits[1],
            self.bits[2],
            if enabled { self.bins() } else { 0 },
        ]
    }
}

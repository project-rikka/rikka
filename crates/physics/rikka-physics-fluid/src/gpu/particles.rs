use rikka_gpu::Compute;
use rikka_gpu::ComputeBuffer;

use crate::error::Result;

pub(crate) const MAX_BONDS: u64 = 12;
pub(crate) const MAX_NEIGHBOURS: u64 = 48;
pub(crate) const MAX_CANDIDATES: u64 = 8;

const VEC4: u64 = 16;
const WORD: u64 = 4;
const HALF4: u64 = 8;
const LOOSE: u32 = u32::MAX;

pub(crate) struct State {
    pub(crate) positions: ComputeBuffer,
    pub(crate) velocities: ComputeBuffer,
    pub(crate) phases: ComputeBuffer,
    pub(crate) warm: ComputeBuffer,
    pub(crate) bond_counts: ComputeBuffer,
    pub(crate) partners: ComputeBuffer,
    pub(crate) rests: ComputeBuffer,
    pub(crate) anchors: ComputeBuffer,
}

impl State {
    fn new(compute: &Compute, capacity: u64) -> Result<State> {
        Ok(State {
            positions: compute.buffer(capacity * VEC4)?,
            velocities: compute.buffer(capacity * VEC4)?,
            phases: compute.buffer(capacity * WORD)?,
            warm: compute.buffer(capacity * VEC4)?,
            bond_counts: compute.buffer(capacity * WORD)?,
            partners: compute.buffer(capacity * MAX_BONDS * WORD)?,
            rests: compute.buffer(capacity * MAX_BONDS * WORD)?,
            anchors: compute.buffer(capacity * VEC4)?,
        })
    }

    pub(crate) fn persistent(&self) -> [(&ComputeBuffer, u64); 6] {
        [
            (&self.positions, VEC4),
            (&self.velocities, VEC4),
            (&self.phases, WORD),
            (&self.warm, VEC4),
            (&self.bond_counts, WORD),
            (&self.anchors, VEC4),
        ]
    }

    pub(crate) fn bond_planes(&self) -> [&ComputeBuffer; 2] {
        [&self.partners, &self.rests]
    }
}

pub(crate) struct Particles {
    pub(crate) states: [State; 2],
    pub(crate) current: usize,
    pub(crate) predicted: [ComputeBuffer; 2],
    pub(crate) normals: ComputeBuffer,
    pub(crate) vorticity_magnitude: ComputeBuffer,
    pub(crate) targets: ComputeBuffer,
    pub(crate) bond_lambdas: ComputeBuffer,
    pub(crate) proposals: ComputeBuffer,
    pub(crate) owned_colours: ComputeBuffer,
    pub(crate) used_colours: [ComputeBuffer; 2],
    pub(crate) top_bonds: [ComputeBuffer; 2],
    pub(crate) nearest: ComputeBuffer,
    pub(crate) dilations: ComputeBuffer,
    pub(crate) foam_counts: ComputeBuffer,
    pub(crate) contacts: ComputeBuffer,
    pub(crate) motion: ComputeBuffer,
    pub(crate) walls: ComputeBuffer,
    pub(crate) clearances: ComputeBuffer,
    pub(crate) cache: ComputeBuffer,
    pub(crate) list: ComputeBuffer,
    pub(crate) listed: ComputeBuffer,
    pub(crate) placeholder: ComputeBuffer,
    pub(crate) stride: u32,
}

impl Particles {
    pub(crate) fn new(compute: &Compute, capacity: u32) -> Result<Particles> {
        let wide = u64::from(capacity);
        Ok(Particles {
            states: [State::new(compute, wide)?, State::new(compute, wide)?],
            current: 0,
            predicted: [compute.buffer(wide * VEC4)?, compute.buffer(wide * VEC4)?],
            normals: compute.buffer(wide * HALF4)?,
            vorticity_magnitude: compute.buffer(wide * WORD)?,
            targets: compute.buffer(wide * VEC4)?,
            bond_lambdas: compute.buffer(wide * MAX_BONDS * WORD)?,
            proposals: compute.buffer(wide * MAX_BONDS * WORD)?,
            owned_colours: compute.buffer(wide * WORD)?,
            used_colours: [compute.buffer(wide * WORD)?, compute.buffer(wide * WORD)?],
            top_bonds: [compute.buffer(wide * WORD)?, compute.buffer(wide * WORD)?],
            nearest: compute.buffer(wide * WORD)?,
            dilations: compute.buffer(wide * WORD)?,
            foam_counts: compute.buffer((wide + 1) * WORD)?,
            contacts: compute.buffer(wide * VEC4)?,
            motion: compute.buffer(wide * VEC4)?,
            walls: compute.buffer(wide * HALF4)?,
            clearances: compute.buffer(wide * WORD)?,
            cache: compute.buffer(wide * MAX_CANDIDATES * WORD)?,
            list: compute.buffer(wide * MAX_NEIGHBOURS * WORD)?,
            listed: compute.buffer(wide * WORD)?,
            placeholder: compute.buffer(VEC4)?,
            stride: capacity,
        })
    }

    pub(crate) fn state(&self) -> &State {
        &self.states[self.current]
    }

    pub(crate) fn other(&self) -> &State {
        &self.states[1 - self.current]
    }

    pub(crate) fn adopt_targets(&mut self) {
        std::mem::swap(&mut self.states[self.current].velocities, &mut self.targets);
    }

    pub(crate) fn loosen(&self, batch: &mut rikka_gpu::Batch) {
        for state in &self.states {
            batch.fill(&state.anchors, LOOSE);
        }
    }

    pub(crate) fn flip(&mut self) {
        self.current = 1 - self.current;
    }
}

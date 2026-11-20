use rikka_gpu::Batch;
use rikka_physics_gpu::Pass;

use crate::gpu::particles::Particles;
use crate::gpu::solver::StepInputs;

const COLOURING_ROUNDS: usize = 4;
const BEFORE_COLOURING: usize = 3;

pub(crate) struct Sweep {
    pub(crate) predicted: usize,
    pub(crate) count: u32,
    pub(crate) colours: u32,
    pub(crate) first: bool,
    pub(crate) walled: bool,
    pub(crate) dt: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct Rebuild {
    pub(crate) state: usize,
    pub(crate) count: u32,
    pub(crate) dt: f32,
    pub(crate) spacing: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct Pending {
    rebuild: Rebuild,
    next: usize,
}

pub(crate) fn gather(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    sorted: u32,
) {
    let from = particles.state();
    let to = particles.other();
    let sorter = step_inputs.sorter;
    Pass::direct(&step_inputs.kernels.gather_bonds, sorted, 128)
        .read(&sorter.perm)
        .read(&sorter.inverse)
        .read(&from.phases)
        .read(&from.bond_counts)
        .read(&from.partners)
        .read(&from.rests)
        .write(&to.bond_counts)
        .write(&to.partners)
        .write(&to.rests)
        .write(&particles.owned_colours)
        .write(&particles.nearest)
        .write(&particles.dilations)
        .read(&to.positions)
        .uint(sorted)
        .uint(particles.stride)
        .run(batch);
}

pub(crate) fn sweep(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    sweep: &Sweep,
) {
    let state = particles.state();
    for colour in 0..sweep.colours {
        let pass = Pass::direct(&step_inputs.kernels.bond_solve, sweep.count, 128)
            .read(step_inputs.settings)
            .write(&particles.predicted[sweep.predicted])
            .read(&state.positions)
            .read(&state.phases)
            .read(&particles.owned_colours)
            .read(&state.partners)
            .read(&state.rests)
            .write(&particles.bond_lambdas);
        let pass = if sweep.walled {
            pass.write(&particles.clearances)
        } else {
            pass.read(&particles.placeholder)
        };
        pass.uint(sweep.count)
            .uint(particles.stride)
            .uint(colour)
            .uint(u32::from(sweep.first))
            .uint(u32::from(sweep.walled))
            .float(sweep.dt)
            .run(batch);
    }
}

impl Rebuild {
    pub(crate) const STAGES: usize = BEFORE_COLOURING + COLOURING_ROUNDS;

    pub(crate) fn stage(
        &self,
        step_inputs: &StepInputs<'_>,
        particles: &Particles,
        batch: &mut Batch,
        stage: usize,
    ) {
        let state = &particles.states[self.state];
        let kernels = step_inputs.kernels;
        let count = self.count;
        match stage {
            0 => Pass::direct(&kernels.bond_relax, count, 128)
                .read(step_inputs.settings)
                .read(&state.positions)
                .read(&state.phases)
                .write(&state.bond_counts)
                .write(&state.partners)
                .write(&state.rests)
                .read(&particles.nearest)
                .uint(count)
                .uint(particles.stride)
                .float(self.dt)
                .run(batch),
            1 => Pass::direct(&kernels.bond_propose, count, 128)
                .read(step_inputs.settings)
                .read(&state.positions)
                .read(&state.phases)
                .read(&particles.list)
                .read(&particles.listed)
                .read(&state.bond_counts)
                .read(&state.partners)
                .read(&state.rests)
                .write(&particles.proposals)
                .uint(count)
                .uint(particles.stride)
                .run(batch),
            2 => Pass::direct(&kernels.bond_accept, count, 128)
                .read(&state.positions)
                .write(&state.bond_counts)
                .write(&state.partners)
                .write(&state.rests)
                .read(&particles.proposals)
                .write(&particles.used_colours[0])
                .write(&particles.top_bonds[0])
                .uint(count)
                .uint(particles.stride)
                .float(self.spacing)
                .run(batch),
            _ => {
                let round = stage - BEFORE_COLOURING;
                let tallied = round + 1 < COLOURING_ROUNDS;
                let (now, next) = (round % 2, 1 - round % 2);
                let pass = Pass::direct(&kernels.bond_colour, count, 128)
                    .read(&state.bond_counts)
                    .write(&state.partners)
                    .read(&particles.used_colours[now])
                    .read(&particles.top_bonds[now]);
                let pass = if tallied {
                    pass.write(&particles.used_colours[next])
                        .write(&particles.top_bonds[next])
                } else {
                    pass.read(&particles.used_colours[now])
                        .read(&particles.top_bonds[now])
                };
                pass.uint(count)
                    .uint(particles.stride)
                    .uint(u32::from(tallied))
                    .run(batch);
            }
        }
    }

    pub(crate) fn from(self, next: usize) -> Option<Pending> {
        (next < Rebuild::STAGES).then_some(Pending {
            rebuild: self,
            next,
        })
    }
}

impl Pending {
    pub(crate) fn stage(
        &mut self,
        step_inputs: &StepInputs<'_>,
        particles: &Particles,
        batch: &mut Batch,
    ) -> bool {
        if self.next >= Rebuild::STAGES {
            return false;
        }
        self.rebuild.stage(step_inputs, particles, batch, self.next);
        self.next += 1;
        true
    }

    pub(crate) fn finish(
        mut self,
        step_inputs: &StepInputs<'_>,
        particles: &Particles,
        batch: &mut Batch,
    ) {
        while self.stage(step_inputs, particles, batch) {}
    }
}

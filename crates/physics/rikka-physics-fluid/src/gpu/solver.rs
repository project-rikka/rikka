use glam::Vec3;
use rikka_gpu::{Batch, ComputeBuffer};
use rikka_physics_gpu::Pass;

use crate::gpu::bodies::BodyBuffers;
use crate::gpu::bonds::{self, Pending, Rebuild, Sweep};
use crate::gpu::kernels::Kernels;
use crate::gpu::particles::Particles;
use crate::gpu::settings::Grid;
use crate::gpu::sorter::{Keys, Sorter, Sorting};
use crate::gpu::viscous::{Field, Viscous};

const BOND_SWEEPS: u32 = 2;
const MODE_SHOWN: u32 = 4;
const MODE_REACT: u32 = 8;

pub(crate) struct Substep {
    pub(crate) sorted: u32,
    pub(crate) live: u32,
    pub(crate) dt: f32,
    pub(crate) alpha: (f32, f32),
    pub(crate) gravity: Vec3,
    pub(crate) iterations: u32,
    pub(crate) sweeps: u32,
    pub(crate) bonds: bool,
    pub(crate) cohesive: bool,
    pub(crate) colours: u32,
    pub(crate) collide: bool,
    pub(crate) has_drapes: bool,
    pub(crate) potentials: bool,
    pub(crate) shown: bool,
    pub(crate) seed: u32,
    pub(crate) max_speed: f32,
    pub(crate) spacing: f32,
}

pub(crate) struct SpawnInputs<'a> {
    pub(crate) spawns: &'a ComputeBuffer,
    pub(crate) first: u32,
    pub(crate) count: u32,
    pub(crate) base: u32,
    pub(crate) alpha: f32,
    pub(crate) inverse_span: f32,
}

pub(crate) struct StepInputs<'a> {
    pub(crate) kernels: &'a Kernels,
    pub(crate) viscous: &'a Viscous,
    pub(crate) settings: &'a ComputeBuffer,
    pub(crate) sorter: &'a Sorter,
    pub(crate) grid: Grid,
    pub(crate) bodies: Option<&'a BodyBuffers>,
}

pub(crate) fn spawn(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    spawn_inputs: &SpawnInputs<'_>,
) {
    let state = particles.state();
    let fallback = &particles.placeholder;
    let (from, to, triangles) = match step_inputs.bodies {
        Some(bodies) => (bodies.skin_from(), bodies.skin_to(), &bodies.triangles),
        None => (fallback, fallback, fallback),
    };
    Pass::direct(&step_inputs.kernels.emit, spawn_inputs.count, 128)
        .read(spawn_inputs.spawns)
        .write(&state.positions)
        .write(&state.velocities)
        .write(&state.phases)
        .write(&state.warm)
        .write(&state.bond_counts)
        .write(&state.anchors)
        .read(from)
        .read(to)
        .read(triangles)
        .uint(spawn_inputs.first)
        .uint(spawn_inputs.count)
        .uint(spawn_inputs.base)
        .float(spawn_inputs.alpha)
        .float(spawn_inputs.inverse_span)
        .run(batch);
}

pub(crate) fn substep(
    step_inputs: &StepInputs<'_>,
    particles: &mut Particles,
    batch: &mut Batch,
    step: &Substep,
    pending: Option<Pending>,
) -> Option<Pending> {
    sort(step_inputs, particles, batch, step, pending);
    gather(step_inputs, particles, batch, step);
    particles.flip();
    let count = step.live;
    neighbours(step_inputs, particles, batch, count);
    if step.collide {
        surfaces(step_inputs, batch, step);
    }
    let walls = match (step_inputs.bodies.is_some(), step.collide) {
        (true, _) => Walls::Stored,
        (false, true) => Walls::Floor,
        (false, false) => Walls::None,
    };
    let mut predicted = 0;
    for iteration in 0..step.iterations {
        let first = iteration == 0;
        if walls == Walls::Stored {
            let mode = if first {
                Collision::Probe
            } else {
                Collision::Between
            };
            collide(step_inputs, particles, batch, predicted, count, step, mode);
        }
        let last = iteration + 1 == step.iterations;
        let shaped = if last && step.potentials {
            NormalOutput::Surface
        } else if (first && step.cohesive) || (last && step.sweeps > 0) {
            NormalOutput::Density
        } else {
            NormalOutput::None
        };
        density(
            step_inputs,
            particles,
            batch,
            (predicted, shaped),
            count,
            walls,
        );
        solve(
            step_inputs,
            particles,
            batch,
            (predicted, walls),
            count,
            first,
            step,
        );
        predicted = 1 - predicted;
        if step.bonds {
            for sweep in 0..BOND_SWEEPS {
                bonds::sweep(
                    step_inputs,
                    particles,
                    batch,
                    &Sweep {
                        predicted,
                        count,
                        colours: step.colours,
                        first: first && sweep == 0,
                        walled: walls == Walls::Stored,
                        dt: step.dt,
                    },
                );
            }
        }
    }
    match walls {
        Walls::Stored => collide(
            step_inputs,
            particles,
            batch,
            predicted,
            count,
            step,
            Collision::Last,
        ),
        Walls::Floor | Walls::None => finish(step_inputs, particles, batch, predicted, count, step),
    }
    velocities(step_inputs, particles, batch, count, step)
}

fn sort(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    step: &Substep,
    mut pending: Option<Pending>,
) {
    let sorting = Sorting {
        sorter: step_inputs.sorter,
        kernels: step_inputs.kernels,
        settings: step_inputs.settings,
        keys: Keys::Simulation {
            state: particles.state(),
            dt: step.dt,
        },
        grid: step_inputs.grid,
        count: step.sorted,
    };
    let mut stage = 0;
    while sorting.stage(batch, stage) {
        if let Some(pending) = &mut pending {
            pending.stage(step_inputs, particles, batch);
        }
        stage += 1;
    }
    gather_state(step_inputs, particles, batch, step);
    if let Some(pending) = pending {
        pending.finish(step_inputs, particles, batch);
    }
}

fn gather_state(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    step: &Substep,
) {
    let from = particles.state();
    let to = particles.other();
    Pass::direct(&step_inputs.kernels.gather_state, step.sorted, 128)
        .read(step_inputs.settings)
        .read(&step_inputs.sorter.perm)
        .read(&from.positions)
        .read(&from.velocities)
        .read(&from.phases)
        .read(&from.warm)
        .write(&to.positions)
        .write(&to.velocities)
        .write(&to.phases)
        .write(&to.warm)
        .write(&particles.predicted[0])
        .read(&from.anchors)
        .write(&to.anchors)
        .uint(step.sorted)
        .float(step.dt)
        .run(batch);
}

fn gather(step_inputs: &StepInputs<'_>, particles: &Particles, batch: &mut Batch, step: &Substep) {
    if step.bonds {
        bonds::gather(step_inputs, particles, batch, step.sorted);
    }
}

fn neighbours(step_inputs: &StepInputs<'_>, particles: &Particles, batch: &mut Batch, count: u32) {
    Pass::direct(&step_inputs.kernels.neighbours, count, 128)
        .read(step_inputs.settings)
        .read(&particles.predicted[0])
        .read(&particles.state().phases)
        .read(&step_inputs.sorter.starts)
        .write(&particles.list)
        .write(&particles.listed)
        .uint(count)
        .uint(particles.stride)
        .run(batch);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Walls {
    None,
    Floor,
    Stored,
}

impl Walls {
    fn code(self) -> u32 {
        match self {
            Walls::None => 0,
            Walls::Floor => 1,
            Walls::Stored => 2,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NormalOutput {
    None,
    Density,
    Surface,
}

impl NormalOutput {
    fn code(self) -> u32 {
        match self {
            NormalOutput::None => 0,
            NormalOutput::Density => 1,
            NormalOutput::Surface => 2,
        }
    }
}

fn density(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    (predicted, shaped): (usize, NormalOutput),
    count: u32,
    walls: Walls,
) {
    let state = particles.state();
    let pass = Pass::direct(&step_inputs.kernels.density, count, 128)
        .read(step_inputs.settings)
        .write(&particles.predicted[predicted])
        .read(&state.phases)
        .read(&particles.list)
        .read(&particles.listed);
    let pass = if shaped != NormalOutput::None {
        pass.write(&particles.normals)
    } else {
        pass.read(&particles.placeholder)
    };
    pass.read(&particles.walls)
        .read(&particles.dilations)
        .uint(count)
        .uint(particles.stride)
        .uint(walls.code())
        .uint(shaped.code())
        .run(batch);
}

fn solve(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    (predicted, walls): (usize, Walls),
    count: u32,
    first: bool,
    step: &Substep,
) {
    let state = particles.state();
    let normals = if first && step.cohesive {
        &particles.normals
    } else {
        &particles.placeholder
    };
    Pass::direct(&step_inputs.kernels.solve, count, 128)
        .read(step_inputs.settings)
        .read(&particles.predicted[predicted])
        .write(&particles.predicted[1 - predicted])
        .read(&state.phases)
        .read(&particles.list)
        .read(&particles.listed)
        .read(normals)
        .read(&particles.walls)
        .write(&particles.clearances)
        .uint(count)
        .uint(u32::from(first))
        .float(step.dt)
        .uint(particles.stride)
        .uint(walls.code())
        .run(batch);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Collision {
    Probe,
    Between,
    Last,
}

impl Collision {
    fn label(self) -> &'static str {
        match self {
            Collision::Probe => "collide_probe",
            Collision::Between => "collide_between",
            Collision::Last => "collide_last",
        }
    }

    fn mode(self) -> u32 {
        match self {
            Collision::Probe => 2,
            Collision::Between => 0,
            Collision::Last => 1,
        }
    }
}

fn collide(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    predicted: usize,
    count: u32,
    step: &Substep,
    collision: Collision,
) {
    let state = particles.state();
    let unused = &particles.placeholder;
    let last = collision == Collision::Last;
    let shown = last && step.shown;
    let react = last && step.has_drapes;
    let pass = Pass::direct(&step_inputs.kernels.collide, count, 128)
        .label(collision.label())
        .read(step_inputs.settings);
    let pass = if last {
        pass.write(&state.positions)
            .read(&particles.predicted[predicted])
    } else {
        pass.read(&state.positions)
            .write(&particles.predicted[predicted])
    };
    let pass = pass.write(&state.phases);
    let pass = match (last, shown) {
        (true, true) => pass.write(&state.velocities).write(&particles.motion),
        (true, false) => pass.write(&state.velocities).read(unused),
        (false, _) => pass.read(unused).read(unused),
    };
    let pass = match step_inputs.bodies {
        Some(bodies) => {
            let pass = pass
                .read(&bodies.table)
                .reads_through(&bodies.faces)
                .reads_through(&bodies.spheres)
                .reads_through(&bodies.triangles)
                .reads_through(&bodies.tri_starts)
                .reads_through(&bodies.tri_items);
            let pass = if last {
                pass.writes_through(&bodies.deposits)
            } else {
                pass
            };
            if react {
                pass.writes_through(&bodies.reactions)
            } else {
                pass
            }
        }
        None => pass.read(unused),
    };
    let pass = match collision {
        Collision::Probe => pass.write(&particles.cache),
        Collision::Between | Collision::Last => pass.read(&particles.cache),
    };
    let pass = if last {
        pass.write(&particles.contacts)
            .read(unused)
            .read(&particles.clearances)
    } else {
        pass.read(unused)
            .write(&particles.walls)
            .write(&particles.clearances)
    };
    let mode =
        collision.mode() | if shown { MODE_SHOWN } else { 0 } | if react { MODE_REACT } else { 0 };
    pass.write(&state.anchors)
        .uint(count)
        .float(step.dt)
        .uint(mode)
        .uint(particles.stride)
        .float(step.max_speed)
        .run(batch);
}

fn surfaces(step_inputs: &StepInputs<'_>, batch: &mut Batch, step: &Substep) {
    let Some(bodies) = step_inputs.bodies else {
        return;
    };
    Pass::direct(&step_inputs.kernels.tri_frame, bodies.triangle_count, 128)
        .read(bodies.skin_from())
        .read(bodies.skin_to())
        .read(&bodies.triangles)
        .write(&bodies.spheres)
        .write(&bodies.faces)
        .uint(bodies.triangle_count)
        .float(step.alpha.0)
        .float(step.alpha.1)
        .run(batch);
}

fn finish(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    predicted: usize,
    count: u32,
    step: &Substep,
) {
    let state = particles.state();
    let motion = if step.shown {
        &particles.motion
    } else {
        &particles.placeholder
    };
    let pass = Pass::direct(&step_inputs.kernels.finish, count, 128)
        .read(step_inputs.settings)
        .write(&state.positions)
        .read(&particles.predicted[predicted])
        .write(&state.phases)
        .write(&state.velocities);
    let pass = if step.shown {
        pass.write(motion)
    } else {
        pass.read(motion)
    };
    let contacts = if step.collide {
        &particles.contacts
    } else {
        &particles.placeholder
    };
    let pass = if step.collide {
        pass.write(contacts)
    } else {
        pass.read(contacts)
    };
    pass.uint(count)
        .float(step.dt)
        .float(step.max_speed)
        .uint(u32::from(step.shown))
        .run(batch);
}

fn velocities(
    step_inputs: &StepInputs<'_>,
    particles: &mut Particles,
    batch: &mut Batch,
    count: u32,
    step: &Substep,
) -> Option<Pending> {
    curl(step_inputs, particles, batch, count, step);
    let rebuild = step.bonds.then_some(Rebuild {
        state: particles.current,
        count,
        dt: step.dt,
        spacing: step.spacing,
    });
    let beside = |batch: &mut Batch, particles: &Particles, stage: usize| {
        if let Some(rebuild) = &rebuild
            && stage < Rebuild::STAGES
        {
            rebuild.stage(step_inputs, particles, batch, stage);
        }
    };
    beside(batch, particles, 0);
    if step.sweeps == 0 {
        particles.adopt_targets();
        Pass::direct(&step_inputs.kernels.settle, count, 128)
            .read(&particles.vorticity_magnitude)
            .write(&particles.state().positions)
            .uint(count)
            .run(batch);
        beside(batch, particles, 1);
        return rebuild.and_then(|rebuild| rebuild.from(2));
    }
    let field = Field {
        kernels: step_inputs.kernels,
        settings: step_inputs.settings,
        state: particles.state(),
        list: &particles.list,
        listed: &particles.listed,
        stride: particles.stride,
        normals: &particles.normals,
        targets: &particles.targets,
        vorticity_magnitude: &particles.vorticity_magnitude,
        contacts: step.collide.then_some(&particles.contacts),
        count,
        dt: step.dt,
        iterations: step.sweeps,
    };
    let stages = Viscous::stages(step.sweeps);
    for stage in 0..stages {
        step_inputs.viscous.stage(batch, &field, stage);
        beside(batch, particles, stage + 1);
    }
    rebuild.and_then(|rebuild| rebuild.from(stages + 1))
}

fn curl(
    step_inputs: &StepInputs<'_>,
    particles: &Particles,
    batch: &mut Batch,
    count: u32,
    step: &Substep,
) {
    let state = particles.state();
    let normals = if step.potentials {
        &particles.normals
    } else {
        &particles.placeholder
    };
    let foam_counts = if step.potentials {
        &particles.foam_counts
    } else {
        &particles.placeholder
    };
    let pass = Pass::direct(&step_inputs.kernels.curl, count, 128)
        .read(step_inputs.settings)
        .read(&state.positions)
        .read(&state.velocities)
        .write(&particles.targets)
        .read(&state.phases)
        .read(&particles.list)
        .read(&particles.listed)
        .write(&particles.vorticity_magnitude)
        .read(normals);
    let pass = if step.potentials {
        pass.write(foam_counts)
    } else {
        pass.read(foam_counts)
    };
    pass.uint(count)
        .float(step.dt)
        .uint(u32::from(step.potentials))
        .uint(step.seed)
        .float(step.gravity.x)
        .float(step.gravity.y)
        .float(step.gravity.z)
        .uint(particles.stride)
        .run(batch);
}

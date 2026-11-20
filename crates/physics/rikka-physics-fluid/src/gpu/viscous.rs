use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer};
use rikka_physics_gpu::Pass;

use crate::error::Result;
use crate::gpu::kernels::Kernels;
use crate::gpu::particles::{MAX_NEIGHBOURS, State};

const VEC4: u64 = 16;
const WORD: u64 = 4;
const WIDTH: u32 = 128;
const REDUCTION_HEADER: u64 = 4;

pub(crate) struct Viscous {
    iterate: ComputeBuffer,
    preconditioned: ComputeBuffer,
    direction: ComputeBuffer,
    product: ComputeBuffer,
    diagonal: ComputeBuffer,
    sums: ComputeBuffer,
    couplings: ComputeBuffer,
}

pub(crate) struct Field<'a> {
    pub(crate) kernels: &'a Kernels,
    pub(crate) settings: &'a ComputeBuffer,
    pub(crate) state: &'a State,
    pub(crate) list: &'a ComputeBuffer,
    pub(crate) listed: &'a ComputeBuffer,
    pub(crate) stride: u32,
    pub(crate) normals: &'a ComputeBuffer,
    pub(crate) targets: &'a ComputeBuffer,
    pub(crate) vorticity_magnitude: &'a ComputeBuffer,
    pub(crate) contacts: Option<&'a ComputeBuffer>,
    pub(crate) count: u32,
    pub(crate) dt: f32,
    pub(crate) iterations: u32,
}

impl Viscous {
    pub(crate) fn new(compute: &Compute, capacity: u32) -> Result<Viscous> {
        let wide = u64::from(capacity);
        let groups = u64::from(capacity.div_ceil(WIDTH));
        Ok(Viscous {
            iterate: compute.buffer(2 * wide * VEC4)?,
            preconditioned: compute.buffer(wide * VEC4)?,
            direction: compute.buffer(wide * VEC4)?,
            product: compute.buffer(wide * VEC4)?,
            diagonal: compute.buffer(6 * wide * WORD)?,
            sums: compute.buffer((REDUCTION_HEADER + 3 * groups) * WORD)?,
            couplings: compute.buffer(MAX_NEIGHBOURS * wide * WORD)?,
        })
    }

    pub(crate) fn prepare(&self, batch: &mut Batch) {
        batch.fill_range(&self.sums, 0, REDUCTION_HEADER * WORD, 0);
    }

    pub(crate) fn stages(iterations: u32) -> usize {
        3 * iterations as usize
    }

    pub(crate) fn stage(&self, batch: &mut Batch, field: &Field<'_>, stage: usize) {
        match stage {
            0 => self.start(batch, field),
            _ => {
                let iteration = (stage / 3) as u32;
                match stage % 3 {
                    0 => self.direction(batch, field, iteration),
                    1 => self.apply(batch, field, iteration),
                    _ => self.update(batch, field, iteration),
                }
            }
        }
    }

    fn contacts<'a>(&'a self, field: &Field<'a>) -> (&'a ComputeBuffer, u32) {
        match field.contacts {
            Some(contacts) => (contacts, 1),
            None => (&self.product, 0),
        }
    }

    fn start(&self, batch: &mut Batch, field: &Field<'_>) {
        let state = field.state;
        let (contacts, boundaries) = self.contacts(field);
        Pass::direct(&field.kernels.visc_start, field.count, WIDTH)
            .read(field.settings)
            .read(&state.positions)
            .read(&state.phases)
            .read(field.list)
            .read(field.listed)
            .read(field.normals)
            .read(field.targets)
            .read(&state.warm)
            .write(&self.iterate)
            .write(&self.preconditioned)
            .write(&self.direction)
            .write(&self.diagonal)
            .write(&self.sums)
            .read(contacts)
            .uint(field.count)
            .uint(field.stride)
            .float(field.dt)
            .uint(boundaries)
            .run(batch);
    }

    fn direction(&self, batch: &mut Batch, field: &Field<'_>, iteration: u32) {
        let state = field.state;
        let now = iteration % 2;
        Pass::direct(&field.kernels.visc_direction, field.count, WIDTH)
            .read(&state.phases)
            .read(&self.preconditioned)
            .write(&self.direction)
            .read(&self.sums)
            .uint(field.count)
            .uint(now)
            .run(batch);
    }

    fn apply(&self, batch: &mut Batch, field: &Field<'_>, iteration: u32) {
        let state = field.state;
        let (contacts, boundaries) = self.contacts(field);
        let now = iteration % 2;
        let pass = Pass::direct(&field.kernels.visc_apply, field.count, WIDTH)
            .read(field.settings)
            .read(&state.positions)
            .read(&state.phases)
            .read(field.list)
            .read(field.listed)
            .read(field.normals)
            .read(&self.direction)
            .write(&self.product)
            .write(&self.sums)
            .read(contacts);
        let pass = match iteration {
            0 => pass.write(&self.couplings),
            _ => pass.read(&self.couplings),
        };
        pass.uint(field.count)
            .uint(field.stride)
            .float(field.dt)
            .uint(u32::from(iteration == 0))
            .uint(boundaries)
            .uint(now)
            .run(batch);
    }

    fn update(&self, batch: &mut Batch, field: &Field<'_>, iteration: u32) {
        let state = field.state;
        let now = iteration % 2;
        let last = iteration + 1 == field.iterations;
        let pass = Pass::direct(&field.kernels.visc_update, field.count, WIDTH)
            .read(&self.direction)
            .read(&self.product)
            .read(&self.diagonal);
        let pass = if last {
            pass.read(&self.iterate)
                .read(&self.preconditioned)
                .read(&self.sums)
                .read(field.targets)
                .write(&state.velocities)
                .write(&state.warm)
                .read(field.vorticity_magnitude)
                .write(&state.positions)
        } else {
            pass.write(&self.iterate)
                .write(&self.preconditioned)
                .write(&self.sums)
                .read(&self.product)
                .read(&self.product)
                .read(&self.product)
                .read(&self.product)
                .read(&self.product)
        };
        pass.uint(field.count)
            .uint(field.stride)
            .uint(u32::from(last))
            .uint(now)
            .run(batch);
    }
}

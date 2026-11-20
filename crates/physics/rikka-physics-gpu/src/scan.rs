use rikka_gpu::{Batch, ComputeBuffer};
use rikka_gpu::{Compute, Result};

use crate::kernel::NamedKernel;
use crate::pass::Pass;

pub const MAX_SCANNED: u32 = 4096 * BLOCK;

const BLOCK: u32 = 1024;

#[derive(Debug)]
pub struct Scan {
    sums: ComputeBuffer,
    local: NamedKernel,
    top: NamedKernel,
    add: NamedKernel,
}

impl Scan {
    pub fn new(compute: &Compute) -> Result<Scan> {
        Ok(Scan {
            sums: compute.buffer(4 * u64::from(MAX_SCANNED / BLOCK))?,
            local: NamedKernel::load(compute, scan_kernels::SCAN_LOCAL_COMP, "scan_local")?,
            top: NamedKernel::load(compute, scan_kernels::SCAN_TOP_COMP, "scan_top")?,
            add: NamedKernel::load(compute, scan_kernels::SCAN_ADD_COMP, "scan_add")?,
        })
    }

    pub fn run(&self, batch: &mut Batch, values: &ComputeBuffer, count: u32) {
        if count == 0 {
            return;
        }
        let groups = count.div_ceil(BLOCK);
        Pass::direct(&self.local, groups * 256, 256)
            .write(values)
            .write(&self.sums)
            .uint(count)
            .run(batch);
        if groups == 1 {
            return;
        }
        Pass::direct(&self.top, 1024, 1024)
            .write(&self.sums)
            .uint(groups)
            .run(batch);
        Pass::direct(&self.add, (groups - 1) * 256, 256)
            .write(values)
            .read(&self.sums)
            .uint(count)
            .run(batch);
    }
}

use rikka_gpu::{ComputeBuffer, Ticket};

#[derive(Clone, Debug)]
pub struct Surface {
    pub particles: ComputeBuffer,
    pub particle_count: u32,
    pub spray: ComputeBuffer,
    pub spray_count: u32,
    pub radius: f32,
    pub ready: Ticket,
}

#[derive(Clone, Debug)]
pub struct Wetting {
    pub buffer: ComputeBuffer,
    pub ready: Ticket,
}

mod band;
mod garment;
mod intrusion;
mod mover;
mod physique;
mod region;
mod scene;
mod series;
mod shell;
mod solve;
mod surface;

const PARALLEL: rikka_parallel::Parallel = rikka_parallel::Parallel::new("rikka-fit");

pub use physique::Physique;

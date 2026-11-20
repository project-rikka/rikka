use super::process::Memory;
use super::stats::FrameTimes;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Steady {
    pub fps: f64,
    pub cpu_ms_per_frame: f64,
    pub main_thread_ms_per_frame: f64,
    pub cpu_percent: f64,
    pub frames: FrameTimes,
    pub gpu_memory: Option<u64>,
    pub memory: Memory,
    pub server_ms_per_frame: f64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Idle {
    pub cpu_percent: f64,
    pub wakeups_per_second: f64,
    pub gpu_memory: Option<u64>,
    pub memory: Memory,
    pub server_percent: f64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Hover {
    pub cpu_percent: f64,
    pub cpu_ms_per_move: f64,
    pub main_thread_ms_per_move: f64,
    pub memory: Memory,
    pub server_percent: f64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Startup {
    pub first_frame_ms: f64,
    pub window: (i32, i32),
    pub memory: Memory,
    pub gpu_memory: Option<u64>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub enum Measurement {
    Steady(Steady),
    Idle(Idle),
    Hover(Hover),
    Startup(Startup),
}

impl Measurement {
    #[must_use]
    pub fn memory(&self) -> Memory {
        match self {
            Measurement::Steady(steady) => steady.memory,
            Measurement::Idle(idle) => idle.memory,
            Measurement::Hover(hover) => hover.memory,
            Measurement::Startup(startup) => startup.memory,
        }
    }
}

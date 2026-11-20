mod flow;
mod keys;

use std::time::Duration;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_reactor::Token;
use rikka_compositor_switcher::{Choosing, Holding, Step};
use rikka_compositor_types::OutputId;

#[derive(Debug, Default)]
pub(crate) struct Switcher {
    phase: Phase,
    serial: u64,
    opening: Option<Token>,
    repeating: Option<Repeating>,
}

#[derive(Debug, Default)]
enum Phase {
    #[default]
    Idle,
    Choosing(Switching),
    Lingering(Holding),
}

#[derive(Debug)]
struct Switching {
    serial: u64,
    output: Option<OutputId>,
    shown: bool,
    choosing: Choosing<WindowId>,
}

#[derive(Debug)]
struct Repeating {
    code: u32,
    step: Step,
    every: Duration,
    token: Token,
}

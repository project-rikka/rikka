mod bounce;
mod grid;
mod probe;
mod scenario;
mod table;

use std::time::Duration;

pub use bounce::Bounce;
pub use grid::{CELL_RADIUS, Cell, GRID_GAP, Grid, GridLayout, Motion};
pub use probe::{PROBE_ENV, Probe, Recording, Stamps, monotonic_ns};
pub use scenario::{CELL_COUNTS, LIST_ROWS, Scenario, ScenarioError};
pub use table::{Row, Table};

pub const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

pub const WINDOW_WIDTH: f32 = 1000.0;

pub const WINDOW_HEIGHT: f32 = 800.0;

pub const ROW_HEIGHT: f32 = 32.0;

pub const ROW_ID_WIDTH: f32 = 80.0;

pub const REMOVE_WIDTH: f32 = 32.0;

pub const ROW_TEXT_SIZE: f32 = 14.0;

pub const SCROLL_STEP: f32 = 32.0;

pub const BACKGROUND: u32 = 0x303030;

pub const ROW_BACKGROUND: u32 = 0x262626;

pub const ROW_HOVERED: u32 = 0x3d5a80;

pub const ROW_SELECTED: u32 = 0x8b2e2e;

pub const TEXT: u32 = 0xffffff;

#[must_use]
pub fn rgb(color: u32) -> [u8; 3] {
    [(color >> 16) as u8, (color >> 8) as u8, color as u8]
}

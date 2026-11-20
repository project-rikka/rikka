use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

pub const GRID_GAP: f32 = 2.0;

pub const CELL_RADIUS: f32 = 3.0;

const TEXT_PER_CELL_HEIGHT: f32 = 0.55;

const SMALLEST_TEXT: f32 = 8.0;

const LARGEST_TEXT: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Anim,
    One,
    Idle,
}

impl Motion {
    pub const ALL: [Motion; 3] = [Motion::Anim, Motion::One, Motion::Idle];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Motion::Anim => "anim",
            Motion::One => "one",
            Motion::Idle => "idle",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Motion> {
        Motion::ALL.into_iter().find(|motion| motion.name() == name)
    }

    #[must_use]
    pub fn animates(self) -> bool {
        self != Motion::Idle
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLayout {
    pub columns: usize,
    pub cell_width: f32,
    pub cell_height: f32,
    pub text_size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub color: u32,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub count: usize,
    pub motion: Motion,
    pub frame: u64,
}

impl Grid {
    #[must_use]
    pub fn new(count: usize, motion: Motion) -> Grid {
        Grid {
            count,
            motion,
            frame: 0,
        }
    }

    pub fn advance(&mut self) {
        if self.motion.animates() {
            self.frame += 1;
        }
    }

    #[must_use]
    pub fn layout(&self) -> GridLayout {
        let columns = ((self.count as f32 * WINDOW_WIDTH / WINDOW_HEIGHT)
            .sqrt()
            .ceil() as usize)
            .max(1);
        let rows = self.count.div_ceil(columns);
        let cell_width = (WINDOW_WIDTH - GRID_GAP) / columns as f32 - GRID_GAP;
        let cell_height = (WINDOW_HEIGHT - GRID_GAP) / rows as f32 - GRID_GAP;
        GridLayout {
            columns,
            cell_width: cell_width.floor(),
            cell_height: cell_height.floor(),
            text_size: (cell_height * TEXT_PER_CELL_HEIGHT)
                .clamp(SMALLEST_TEXT, LARGEST_TEXT)
                .floor(),
        }
    }

    #[must_use]
    pub fn cell(&self, index: usize) -> Cell {
        let frame = self.shown_frame(index);
        Cell {
            color: color(index, frame),
            label: ((index as u64 * 13 + frame) % 1000).to_string(),
        }
    }

    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        (0..self.count).map(|index| self.cell(index))
    }

    fn shown_frame(&self, index: usize) -> u64 {
        match self.motion {
            Motion::One if index != 0 => 0,
            _ => self.frame,
        }
    }
}

fn color(index: usize, frame: u64) -> u32 {
    let hue = ((index as u64 * 7 + frame * 3) % 360) as f32 / 60.0;
    let (saturation, value) = (0.55f32, 0.75f32);
    let chroma = value * saturation;
    let second = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (red, green, blue) = match hue as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let lift = value - chroma;
    let byte = |channel: f32| ((channel + lift) * 255.0).round() as u32;
    (byte(red) << 16) | (byte(green) << 8) | byte(blue)
}

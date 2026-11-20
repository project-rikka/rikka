use std::collections::VecDeque;
use std::time::Duration;

const ATTACK: Duration = Duration::from_millis(20);
const HANGOVER: Duration = Duration::from_millis(400);
const BLOCK: Duration = Duration::from_millis(500);
const BLOCKS: usize = 6;
const MARGIN_DB: f32 = 12.0;
const LOWEST_DB: f32 = -70.0;
const HIGHEST_DB: f32 = -20.0;
const UNHEARD_DB: f32 = -50.0;
const OPENING_PROBABILITY: f32 = 0.6;
const HOLDING_PROBABILITY: f32 = 0.35;
const AUDIBLE_DB: f32 = -65.0;

#[derive(Debug)]
pub(crate) struct Vad {
    threshold: Threshold,
    open: bool,
    voiced: Duration,
    quiet: Duration,
}

#[derive(Debug)]
enum Threshold {
    Fixed(f32),
    Automatic(NoiseFloor),
}

impl Vad {
    pub(crate) fn fixed(threshold_db: f32) -> Vad {
        Vad::with(Threshold::Fixed(threshold_db))
    }

    pub(crate) fn automatic() -> Vad {
        Vad::with(Threshold::Automatic(NoiseFloor::new()))
    }

    fn with(threshold: Threshold) -> Vad {
        Vad {
            threshold,
            open: false,
            voiced: Duration::ZERO,
            quiet: Duration::ZERO,
        }
    }

    pub(crate) fn update(
        &mut self,
        level_db: f32,
        probability: Option<f32>,
        frame: Duration,
    ) -> bool {
        let voiced = match &mut self.threshold {
            Threshold::Fixed(threshold) => level_db >= *threshold,
            Threshold::Automatic(floor) => {
                floor.observe(level_db, frame);
                match probability {
                    Some(probability) => {
                        let required_probability = if self.open {
                            HOLDING_PROBABILITY
                        } else {
                            OPENING_PROBABILITY
                        };
                        probability >= required_probability && level_db >= AUDIBLE_DB
                    }
                    None => level_db >= floor.threshold_db(),
                }
            }
        };
        if voiced {
            self.voiced += frame;
            self.quiet = Duration::ZERO;
            if self.voiced >= ATTACK {
                self.open = true;
            }
        } else {
            self.voiced = Duration::ZERO;
            if self.open {
                self.quiet += frame;
                if self.quiet >= HANGOVER {
                    self.open = false;
                }
            }
        }
        self.open
    }
}

#[derive(Debug)]
struct NoiseFloor {
    blocks: VecDeque<f32>,
    lowest: f32,
    elapsed: Duration,
}

impl NoiseFloor {
    fn new() -> NoiseFloor {
        NoiseFloor {
            blocks: VecDeque::with_capacity(BLOCKS + 1),
            lowest: f32::INFINITY,
            elapsed: Duration::ZERO,
        }
    }

    fn observe(&mut self, level_db: f32, frame: Duration) {
        self.lowest = self.lowest.min(level_db);
        self.elapsed += frame;
        if self.elapsed >= BLOCK {
            self.blocks.push_back(self.lowest);
            if self.blocks.len() > BLOCKS {
                self.blocks.pop_front();
            }
            self.lowest = f32::INFINITY;
            self.elapsed = Duration::ZERO;
        }
    }

    fn threshold_db(&self) -> f32 {
        let floor = self.blocks.iter().copied().fold(self.lowest, f32::min);
        if !floor.is_finite() {
            return UNHEARD_DB;
        }
        (floor + MARGIN_DB).clamp(LOWEST_DB, HIGHEST_DB)
    }
}

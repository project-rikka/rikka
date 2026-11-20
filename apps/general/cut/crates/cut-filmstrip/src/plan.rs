#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Continuous,
    Head,
    HeadAndTail,
    Off,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Continuous, Mode::Head, Mode::HeadAndTail, Mode::Off];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footage {
    pub start: i64,
    pub end: i64,
    pub source_in: i64,
    pub speed: f64,
    pub last: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub first: f64,
    pub per_frame: f64,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub left: f32,
    pub right: f32,
    pub source: i64,
    pub tolerance: i64,
    pub tail: bool,
}

impl Footage {
    fn source_at(&self, frame: f64) -> f64 {
        self.source_in as f64 + (frame - self.start as f64) * self.speed
    }

    fn frame_of(&self, source: f64) -> f64 {
        self.start as f64 + (source - self.source_in as f64) / self.speed
    }

    fn clamped_source(&self, source: i64) -> i64 {
        source.clamp(0, self.last.max(0))
    }

    fn final_source(&self) -> i64 {
        self.clamped_source(self.source_at(self.end as f64).ceil() as i64 - 1)
    }
}

impl Window {
    fn x_of(&self, frame: f64) -> f32 {
        ((frame - self.first) * self.per_frame) as f32
    }

    fn frame_at(&self, x: f32) -> f64 {
        self.first + f64::from(x) / self.per_frame
    }
}

#[must_use]
pub fn tiles(mode: Mode, footage: Footage, window: Window, thumbnail_width: f32) -> Vec<Tile> {
    let usable = footage.end > footage.start
        && footage.speed > 0.0
        && window.per_frame > 0.0
        && thumbnail_width > 0.0
        && window.width > 0.0;
    if !usable {
        return Vec::new();
    }
    let left = window.x_of(footage.start as f64);
    let right = window.x_of(footage.end as f64);
    let (seen_left, seen_right) = (left.max(0.0), right.min(window.width));
    if seen_right <= seen_left {
        return Vec::new();
    }
    let head = Tile {
        left,
        right: right.min(left + thumbnail_width),
        source: footage.clamped_source(footage.source_in),
        tolerance: 0,
        tail: false,
    };
    let visible = |tile: &Tile| tile.right > seen_left && tile.left < seen_right;
    match mode {
        Mode::Off => Vec::new(),
        Mode::Head => [head].into_iter().filter(visible).collect(),
        Mode::HeadAndTail => {
            let tail = Tile {
                left: right - thumbnail_width,
                right,
                source: footage.final_source(),
                tolerance: 0,
                tail: true,
            };
            let both = right - left >= thumbnail_width * 2.0;
            [Some(head), both.then_some(tail)]
                .into_iter()
                .flatten()
                .filter(visible)
                .collect()
        }
        Mode::Continuous => continuous(footage, window, thumbnail_width, (seen_left, seen_right)),
    }
}

fn continuous(
    footage: Footage,
    window: Window,
    thumbnail_width: f32,
    visible: (f32, f32),
) -> Vec<Tile> {
    let per_source = window.per_frame / footage.speed;
    let mut step: i64 = 1;
    while (step * 2) as f64 * per_source <= f64::from(thumbnail_width) && step < 1 << 40 {
        step *= 2;
    }
    let x_of = |source: i64| window.x_of(footage.frame_of(source as f64));
    let clip_right = window.x_of(footage.end as f64);
    let final_source = footage.final_source();
    let first_grid = footage.source_in.div_euclid(step) + 1;
    let seen_source = footage.source_at(window.frame_at(visible.0)).floor() as i64;
    let mut index = first_grid.max(seen_source.div_euclid(step));
    let mut tiles = Vec::new();
    let head_right = x_of(first_grid * step).min(clip_right);
    if head_right > visible.0 {
        tiles.push(Tile {
            left: window.x_of(footage.start as f64),
            right: head_right,
            source: footage.clamped_source(footage.source_in),
            tolerance: 0,
            tail: false,
        });
    }
    loop {
        let source = index * step;
        let left = x_of(source);
        if left >= visible.1 || left >= clip_right {
            break;
        }
        let clamped = footage.clamped_source(source);
        tiles.push(Tile {
            left,
            right: x_of(source + step).min(clip_right),
            source: clamped,
            tolerance: match clamped == source {
                true => step.min(final_source - source + 1).max(0),
                false => 0,
            },
            tail: false,
        });
        index += 1;
    }
    tiles
}

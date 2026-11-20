use crate::order::Rect;

pub const LONGEST: u32 = 1024;
pub const MAX_SIDE: u32 = 1568;
pub const MAX_PATCHES: u32 = 1568;
pub const PATCH: u32 = 28;
pub(crate) const MAX_ZOOM: f64 = 8.0;
pub(crate) const GAP: u32 = 8;
const NICE: [u32; 3] = [1, 2, 5];
const GRID_LINES: u32 = 10;
const MIN_GRID_SPACING: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    pub region: Rect,
    pub width: u32,
    pub height: u32,
    pub panels: u32,
}

impl Framing {
    #[must_use]
    pub fn fit(region: Rect, longest: u32, panels: u32) -> Framing {
        let panels = panels.max(1);
        let (across, down) = (f64::from(region.width), f64::from(region.height));
        let widest = across.max(down).max(1.0);
        let mut longest = longest.clamp(1, MAX_SIDE);
        loop {
            let scale = (f64::from(longest) / widest).min(MAX_ZOOM);
            let width = ((across * scale).round() as u32).max(1);
            let height = ((down * scale).round() as u32).max(1);
            let framing = Framing {
                region,
                width,
                height,
                panels,
            };
            if framing.fits() || longest == 1 {
                return framing;
            }
            longest -= 1;
        }
    }

    #[must_use]
    pub fn sheet(&self) -> (u32, u32) {
        (
            self.width * self.panels + GAP * (self.panels - 1),
            self.height,
        )
    }

    #[must_use]
    pub fn fits(&self) -> bool {
        let (width, height) = self.sheet();
        width <= MAX_SIDE
            && height <= MAX_SIDE
            && width.div_ceil(PATCH) * height.div_ceil(PATCH) <= MAX_PATCHES
    }

    #[must_use]
    pub fn scale(&self) -> f64 {
        f64::from(self.width) / f64::from(self.region.width.max(1))
    }

    #[must_use]
    pub fn enlarges(&self) -> bool {
        self.width > self.region.width
    }

    #[must_use]
    pub fn image_x(&self, x: f64) -> f64 {
        (x - f64::from(self.region.left)) * f64::from(self.width) / f64::from(self.region.width)
    }

    #[must_use]
    pub fn image_y(&self, y: f64) -> f64 {
        (y - f64::from(self.region.top)) * f64::from(self.height) / f64::from(self.region.height)
    }

    #[must_use]
    pub fn mapping(&self) -> String {
        let Rect {
            left,
            top,
            width,
            height,
        } = self.region;
        let (right, bottom) = (left + width, top + height);
        let across = number(self.scale());
        let down = number(f64::from(self.height) / f64::from(height.max(1)));
        format!(
            "document x {left}–{right} and y {top}–{bottom} at scale {across} (image x = (document x − {left}) × {across}, image y = (document y − {top}) × {down})"
        )
    }

    pub fn grid_step(&self, requested: Option<u32>) -> Result<u32, String> {
        let scale = self.scale();
        if let Some(step) = requested {
            let spacing = f64::from(step) * scale;
            if step == 0 || spacing < MIN_GRID_SPACING {
                let least = (MIN_GRID_SPACING / scale).ceil() as u32;
                return Err(format!(
                    "grid lines every {step} px would sit {} image px apart at scale {}; use a step of at least {least} or view a smaller region",
                    number(spacing),
                    number(scale)
                ));
            }
            return Ok(step);
        }
        let longest = self.region.width.max(self.region.height);
        let mut magnitude = 1u32;
        loop {
            for nice in NICE {
                let step = nice.saturating_mul(magnitude);
                if longest / step <= GRID_LINES && f64::from(step) * scale >= MIN_GRID_SPACING {
                    return Ok(step);
                }
            }
            magnitude = magnitude.saturating_mul(10);
        }
    }
}

#[must_use]
pub fn number(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let text = format!("{rounded:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

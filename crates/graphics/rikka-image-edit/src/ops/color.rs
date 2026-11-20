pub use rikka_color_lookup::ColorLookup;
use rikka_types::Color;

use crate::error::Result;
use crate::ops::matrix::{Matrix, apply, inverse, product};
use crate::ops::passes::adjust;
use crate::ops::{Effect, kind};
use crate::picture::Frame;
use crate::work::Work;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hues {
    Reds,
    Yellows,
    Greens,
    Cyans,
    Blues,
    Magentas,
}

impl Hues {
    const ALL: [Hues; 6] = [
        Hues::Reds,
        Hues::Yellows,
        Hues::Greens,
        Hues::Cyans,
        Hues::Blues,
        Hues::Magentas,
    ];

    fn range(self) -> [f32; 4] {
        match self {
            Hues::Reds => [315.0, 345.0, 15.0, 45.0],
            Hues::Yellows => [15.0, 45.0, 75.0, 105.0],
            Hues::Greens => [75.0, 105.0, 135.0, 165.0],
            Hues::Cyans => [135.0, 165.0, 195.0, 225.0],
            Hues::Blues => [195.0, 225.0, 255.0, 285.0],
            Hues::Magentas => [255.0, 285.0, 315.0, 345.0],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shift {
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

impl Shift {
    #[must_use]
    pub fn new(hue: f32, saturation: f32, lightness: f32) -> Shift {
        Shift {
            hue: hue.clamp(-180.0, 180.0),
            saturation: saturation.clamp(-100.0, 100.0),
            lightness: lightness.clamp(-100.0, 100.0),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HueSaturation {
    master: Shift,
    ranges: [Shift; 6],
    colorize: Option<(f32, f32, f32)>,
}

impl HueSaturation {
    #[must_use]
    pub fn new() -> HueSaturation {
        HueSaturation::default()
    }

    #[must_use]
    pub fn master(mut self, shift: Shift) -> HueSaturation {
        self.master = shift;
        self
    }

    #[must_use]
    pub fn hues(mut self, hues: Hues, shift: Shift) -> HueSaturation {
        self.ranges[hues as usize] = shift;
        self
    }

    #[must_use]
    pub fn colorize(mut self, hue: f32, saturation: f32, lightness: f32) -> HueSaturation {
        self.colorize = Some((
            hue.rem_euclid(360.0),
            saturation.clamp(0.0, 100.0),
            lightness.clamp(-100.0, 100.0),
        ));
        self
    }
}

impl Effect for HueSaturation {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let (colorize, hue, saturation, lightness) = match self.colorize {
            Some((hue, saturation, lightness)) => (1.0, hue, saturation / 100.0, lightness / 100.0),
            None => (0.0, 0.0, 0.0, 0.0),
        };
        let mut params = vec![
            colorize,
            hue,
            saturation,
            lightness,
            self.master.hue,
            self.master.saturation / 100.0,
            self.master.lightness / 100.0,
        ];
        for hues in Hues::ALL {
            let shift = self.ranges[hues as usize];
            params.extend(hues.range());
            params.extend([shift.hue, shift.saturation / 100.0, shift.lightness / 100.0]);
        }
        adjust(work, frame, kind::HUE_SATURATION, &params, 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorBalance {
    shadows: [f32; 3],
    midtones: [f32; 3],
    highlights: [f32; 3],
    preserve_luminosity: bool,
}

impl Default for ColorBalance {
    fn default() -> ColorBalance {
        ColorBalance {
            shadows: [0.0; 3],
            midtones: [0.0; 3],
            highlights: [0.0; 3],
            preserve_luminosity: true,
        }
    }
}

impl ColorBalance {
    #[must_use]
    pub fn new() -> ColorBalance {
        ColorBalance::default()
    }

    #[must_use]
    pub fn shadows(mut self, cyan_red: f32, magenta_green: f32, yellow_blue: f32) -> ColorBalance {
        self.shadows = balance(cyan_red, magenta_green, yellow_blue);
        self
    }

    #[must_use]
    pub fn midtones(mut self, cyan_red: f32, magenta_green: f32, yellow_blue: f32) -> ColorBalance {
        self.midtones = balance(cyan_red, magenta_green, yellow_blue);
        self
    }

    #[must_use]
    pub fn highlights(
        mut self,
        cyan_red: f32,
        magenta_green: f32,
        yellow_blue: f32,
    ) -> ColorBalance {
        self.highlights = balance(cyan_red, magenta_green, yellow_blue);
        self
    }

    #[must_use]
    pub fn preserve_luminosity(mut self, preserve: bool) -> ColorBalance {
        self.preserve_luminosity = preserve;
        self
    }
}

fn balance(a: f32, b: f32, c: f32) -> [f32; 3] {
    [a, b, c].map(|value| value.clamp(-100.0, 100.0))
}

impl Effect for ColorBalance {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let mut params: Vec<f32> = [self.shadows, self.midtones, self.highlights]
            .into_iter()
            .flatten()
            .map(|value| value / 100.0)
            .collect();
        params.push(f32::from(u8::from(self.preserve_luminosity)));
        adjust(work, frame, kind::COLOR_BALANCE, &params, 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlackWhite {
    weights: [f32; 6],
    tint: Option<Color>,
}

impl Default for BlackWhite {
    fn default() -> BlackWhite {
        BlackWhite {
            weights: [40.0, 60.0, 40.0, 60.0, 20.0, 80.0],
            tint: None,
        }
    }
}

impl BlackWhite {
    #[must_use]
    pub fn new() -> BlackWhite {
        BlackWhite::default()
    }

    #[must_use]
    pub fn weight(mut self, hues: Hues, percent: f32) -> BlackWhite {
        self.weights[hues as usize] = percent.clamp(-200.0, 300.0);
        self
    }

    #[must_use]
    pub fn tint(mut self, tint: Color) -> BlackWhite {
        self.tint = Some(tint);
        self
    }
}

impl Effect for BlackWhite {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let mut params: Vec<f32> = self.weights.iter().map(|value| value / 100.0).collect();
        match self.tint {
            Some(tint) => {
                let [r, g, b, _] = frame.space.translated([tint.r, tint.g, tint.b, 1.0]);
                params.extend([1.0, r, g, b]);
            }
            None => params.extend([0.0; 4]),
        }
        adjust(work, frame, kind::BLACK_WHITE, &params, 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sepia {
    tone: BlackWhite,
}

impl Default for Sepia {
    fn default() -> Sepia {
        Sepia {
            tone: BlackWhite::new().tint(Color::rgb(0.74, 0.58, 0.40)),
        }
    }
}

impl Sepia {
    #[must_use]
    pub fn new() -> Sepia {
        Sepia::default()
    }
}

impl Effect for Sepia {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        self.tone.apply(work, frame)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhotoFilter {
    color: Color,
    density: f32,
    preserve_luminosity: bool,
}

impl Default for PhotoFilter {
    fn default() -> PhotoFilter {
        PhotoFilter::new(Color::rgb(236.0 / 255.0, 138.0 / 255.0, 0.0))
    }
}

impl PhotoFilter {
    #[must_use]
    pub fn new(color: Color) -> PhotoFilter {
        PhotoFilter {
            color,
            density: 25.0,
            preserve_luminosity: true,
        }
    }

    #[must_use]
    pub fn density(mut self, percent: f32) -> PhotoFilter {
        self.density = percent.clamp(1.0, 100.0);
        self
    }

    #[must_use]
    pub fn preserve_luminosity(mut self, preserve: bool) -> PhotoFilter {
        self.preserve_luminosity = preserve;
        self
    }
}

impl Effect for PhotoFilter {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let [r, g, b, _] = frame
            .space
            .translated([self.color.r, self.color.g, self.color.b, 1.0]);
        let params = [
            r,
            g,
            b,
            self.density / 100.0,
            f32::from(u8::from(self.preserve_luminosity)),
        ];
        adjust(work, frame, kind::PHOTO_FILTER, &params, 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primary {
    Red,
    Green,
    Blue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelMixer {
    rows: [[f32; 4]; 3],
    monochrome: Option<[f32; 4]>,
}

impl Default for ChannelMixer {
    fn default() -> ChannelMixer {
        ChannelMixer {
            rows: [
                [100.0, 0.0, 0.0, 0.0],
                [0.0, 100.0, 0.0, 0.0],
                [0.0, 0.0, 100.0, 0.0],
            ],
            monochrome: None,
        }
    }
}

impl ChannelMixer {
    #[must_use]
    pub fn new() -> ChannelMixer {
        ChannelMixer::default()
    }

    #[must_use]
    pub fn output(mut self, primary: Primary, mix: [f32; 3], constant: f32) -> ChannelMixer {
        self.rows[primary as usize] = mixing(mix, constant);
        self
    }

    #[must_use]
    pub fn monochrome(mut self, mix: [f32; 3], constant: f32) -> ChannelMixer {
        self.monochrome = Some(mixing(mix, constant));
        self
    }
}

fn mixing(mix: [f32; 3], constant: f32) -> [f32; 4] {
    [
        mix[0].clamp(-200.0, 200.0),
        mix[1].clamp(-200.0, 200.0),
        mix[2].clamp(-200.0, 200.0),
        constant.clamp(-200.0, 200.0),
    ]
}

impl Effect for ChannelMixer {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let mut params = vec![f32::from(u8::from(self.monochrome.is_some()))];
        for row in self.rows {
            params.extend(row.map(|value| value / 100.0));
        }
        params.extend(
            self.monochrome
                .unwrap_or_default()
                .map(|value| value / 100.0),
        );
        adjust(work, frame, kind::CHANNEL_MIXER, &params, 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    pub at: f32,
    pub color: Color,
    pub midpoint: f32,
}

impl GradientStop {
    #[must_use]
    pub fn new(at: f32, color: Color) -> GradientStop {
        GradientStop {
            at: at.clamp(0.0, 1.0),
            color,
            midpoint: 0.5,
        }
    }

    #[must_use]
    pub fn midpoint(mut self, midpoint: f32) -> GradientStop {
        self.midpoint = midpoint.clamp(0.05, 0.95);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GradientMap {
    stops: Vec<GradientStop>,
    reversed: bool,
}

impl Default for GradientMap {
    fn default() -> GradientMap {
        GradientMap::new([
            GradientStop::new(0.0, Color::rgb(0.0, 0.0, 0.0)),
            GradientStop::new(1.0, Color::rgb(1.0, 1.0, 1.0)),
        ])
    }
}

const GRADIENT_STEPS: usize = 1024;

impl GradientMap {
    #[must_use]
    pub fn new(stops: impl IntoIterator<Item = GradientStop>) -> GradientMap {
        let mut stops: Vec<GradientStop> = stops.into_iter().collect();
        stops.sort_by(|a, b| a.at.total_cmp(&b.at));
        GradientMap {
            stops,
            reversed: false,
        }
    }

    #[must_use]
    pub fn reversed(mut self, reversed: bool) -> GradientMap {
        self.reversed = reversed;
        self
    }

    #[must_use]
    pub fn color_at(&self, t: f32) -> Color {
        let stops: Vec<(f32, f32, [f32; 3])> = self
            .stops
            .iter()
            .map(|stop| {
                (
                    stop.at,
                    stop.midpoint,
                    [stop.color.r, stop.color.g, stop.color.b],
                )
            })
            .collect();
        let t = t.clamp(0.0, 1.0);
        let along = match self.reversed {
            true => 1.0 - t,
            false => t,
        };
        let [r, g, b] = GradientMap::sample(&stops, along);
        Color::rgb(r, g, b)
    }

    fn sample(stops: &[(f32, f32, [f32; 3])], t: f32) -> [f32; 3] {
        let Some(first) = stops.first() else {
            return [t; 3];
        };
        if t <= first.0 {
            return first.2;
        }
        for pair in stops.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if t <= b.0 {
                let local = (t - a.0) / (b.0 - a.0).max(1e-6);
                let eased = match local < a.1 {
                    true => 0.5 * local / a.1,
                    false => 0.5 + 0.5 * (local - a.1) / (1.0 - a.1),
                };
                return [0, 1, 2]
                    .map(|channel| a.2[channel] + (b.2[channel] - a.2[channel]) * eased);
            }
        }
        stops.last().map_or([t; 3], |stop| stop.2)
    }
}

impl Effect for GradientMap {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let stops: Vec<(f32, f32, [f32; 3])> = self
            .stops
            .iter()
            .map(|stop| {
                let [r, g, b, _] =
                    frame
                        .space
                        .translated([stop.color.r, stop.color.g, stop.color.b, 1.0]);
                (stop.at, stop.midpoint, [r, g, b])
            })
            .collect();
        let mut params = vec![f32::from(u8::from(self.reversed))];
        let last = (GRADIENT_STEPS - 1) as f32;
        for step in 0..GRADIENT_STEPS {
            params.extend(GradientMap::sample(&stops, step as f32 / last));
        }
        adjust(
            work,
            frame,
            kind::GRADIENT_MAP,
            &params,
            GRADIENT_STEPS as u32,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Colors {
    Reds,
    Yellows,
    Greens,
    Cyans,
    Blues,
    Magentas,
    Whites,
    Neutrals,
    Blacks,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectiveColor {
    plates: [[f32; 4]; 9],
    absolute: bool,
}

impl SelectiveColor {
    #[must_use]
    pub fn new() -> SelectiveColor {
        SelectiveColor::default()
    }

    #[must_use]
    pub fn colors(
        mut self,
        colors: Colors,
        cyan: f32,
        magenta: f32,
        yellow: f32,
        black: f32,
    ) -> SelectiveColor {
        self.plates[colors as usize] =
            [cyan, magenta, yellow, black].map(|value| value.clamp(-100.0, 100.0));
        self
    }

    #[must_use]
    pub fn absolute(mut self, absolute: bool) -> SelectiveColor {
        self.absolute = absolute;
        self
    }
}

impl Effect for SelectiveColor {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let mut params = vec![f32::from(u8::from(self.absolute)), 0.0, 0.0, 0.0, 0.0];
        for plate in self.plates {
            params.extend(plate.map(|value| value / 100.0));
        }
        adjust(work, frame, kind::SELECTIVE_COLOR, &params, 0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vibrance {
    vibrance: f32,
    saturation: f32,
}

impl Vibrance {
    #[must_use]
    pub fn new(vibrance: f32) -> Vibrance {
        Vibrance {
            vibrance: vibrance.clamp(-100.0, 100.0),
            saturation: 0.0,
        }
    }

    #[must_use]
    pub fn saturation(mut self, saturation: f32) -> Vibrance {
        self.saturation = saturation.clamp(-100.0, 100.0);
        self
    }
}

impl Effect for Vibrance {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let params = [self.vibrance / 100.0, self.saturation / 100.0];
        adjust(work, frame, kind::VIBRANCE, &params, 0)
    }
}

impl Effect for ColorLookup {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let domain = self.domain();
        let mut params = Vec::with_capacity(7 + self.table().len());
        params.push(self.size() as f32);
        params.extend(domain.min);
        params.extend(domain.max);
        params.extend_from_slice(self.table());
        adjust(work, frame, kind::LOOKUP, &params, 0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Desaturate {
    #[default]
    Lightness,
    Luminosity,
    Average,
}

impl Effect for Desaturate {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        adjust(work, frame, kind::DESATURATE, &[*self as u8 as f32], 0)
    }
}

const NEUTRAL_KELVIN: f32 = 6504.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WhiteBalance {
    temperature: f32,
    tint: f32,
}

impl Default for WhiteBalance {
    fn default() -> WhiteBalance {
        WhiteBalance {
            temperature: NEUTRAL_KELVIN,
            tint: 0.0,
        }
    }
}

impl WhiteBalance {
    #[must_use]
    pub fn new(temperature: f32) -> WhiteBalance {
        WhiteBalance {
            temperature: temperature.clamp(1667.0, 25000.0),
            tint: 0.0,
        }
    }

    #[must_use]
    pub fn tint(mut self, tint: f32) -> WhiteBalance {
        self.tint = tint.clamp(-150.0, 150.0);
        self
    }

    pub(crate) fn matrix(&self, to_xyz: Matrix) -> Matrix {
        let reference = xyz(white(f64::from(NEUTRAL_KELVIN), 0.0));
        let scene = xyz(white(
            f64::from(self.temperature),
            f64::from(self.tint) / 3000.0,
        ));
        let d50 = [0.9642, 1.0, 0.8251];
        let placed = apply(&adaptation(reference, d50), scene);
        let correction = adaptation(placed, d50);
        product(&product(&inverse(&to_xyz), &correction), &to_xyz)
    }
}

impl Effect for WhiteBalance {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let matrix = self.matrix(frame.space.to_xyz());
        let params: Vec<f32> = matrix.iter().flatten().map(|&value| value as f32).collect();
        adjust(work, frame, kind::WHITE_BALANCE, &params, 0)
    }
}

fn white(kelvin: f64, duv: f64) -> [f64; 2] {
    let locus = |t: f64| {
        let (t2, t3) = (t * t, t * t * t);
        let x = match t <= 4000.0 {
            true => -0.266_123_9e9 / t3 - 0.234_358_9e6 / t2 + 0.877_695_6e3 / t + 0.179_910,
            false => -3.025_846_9e9 / t3 + 2.107_037_9e6 / t2 + 0.222_634_7e3 / t + 0.240_390,
        };
        let (x2, x3) = (x * x, x * x * x);
        let y = match t {
            t if t <= 2222.0 => {
                -1.106_381_4 * x3 - 1.348_110_20 * x2 + 2.185_558_32 * x - 0.202_196_83
            }
            t if t <= 4000.0 => {
                -0.954_947_6 * x3 - 1.374_185_93 * x2 + 2.091_370_15 * x - 0.167_488_67
            }
            _ => 3.081_758_0 * x3 - 5.873_386_70 * x2 + 3.751_129_97 * x - 0.370_014_83,
        };
        uv([x, y])
    };
    let on_locus = locus(kelvin);
    let next_on_locus = locus(kelvin + 1.0);
    let along = [
        next_on_locus[0] - on_locus[0],
        next_on_locus[1] - on_locus[1],
    ];
    let length = (along[0] * along[0] + along[1] * along[1])
        .sqrt()
        .max(1e-12);
    let normal = [-along[1] / length, along[0] / length];
    let shifted = [on_locus[0] + normal[0] * duv, on_locus[1] + normal[1] * duv];
    xy(shifted)
}

fn uv([x, y]: [f64; 2]) -> [f64; 2] {
    let d = -2.0 * x + 12.0 * y + 3.0;
    [4.0 * x / d, 6.0 * y / d]
}

fn xy([u, v]: [f64; 2]) -> [f64; 2] {
    let d = 2.0 * u - 8.0 * v + 4.0;
    [3.0 * u / d, 2.0 * v / d]
}

fn xyz([x, y]: [f64; 2]) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

const BRADFORD: Matrix = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

fn adaptation(from: [f64; 3], to: [f64; 3]) -> Matrix {
    let source = apply(&BRADFORD, from);
    let target = apply(&BRADFORD, to);
    let scale = [
        [target[0] / source[0], 0.0, 0.0],
        [0.0, target[1] / source[1], 0.0],
        [0.0, 0.0, target[2] / source[2]],
    ];
    product(&product(&inverse(&BRADFORD), &scale), &BRADFORD)
}

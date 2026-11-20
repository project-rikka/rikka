mod apply;

use rikka_psd::{
    Adjustment, AdjustmentLayer, BlackWhite, ColorLookup, CurveData, Curves, GradientMap,
    HueSaturation, Levels, LutFormat, PhotoFilter, PhotoFilterColor, Spline, Vibrance, hsl_to_rgb,
    lab_to_rgb, rgb_to_hsl,
};

use crate::blend::{lum, set_lum};
use crate::table::sample;

const LUT: usize = 1024;

type Lut = Vec<f32>;

fn lut(function: impl Fn(f32) -> f32) -> Lut {
    (0..LUT)
        .map(|index| function(index as f32 / (LUT - 1) as f32).clamp(0.0, 1.0))
        .collect()
}

fn level_lut(record: &rikka_psd::LevelRecord) -> Lut {
    let in_black = f32::from(record.input_floor) / 255.0;
    let in_white = f32::from(record.input_ceiling) / 255.0;
    let out_black = f32::from(record.output_floor) / 255.0;
    let out_white = f32::from(record.output_ceiling) / 255.0;
    let gamma = (f32::from(record.gamma) / 100.0).clamp(0.01, 9.99);
    let range = if (in_white - in_black).abs() > 1e-6 {
        in_white - in_black
    } else {
        1.0
    };
    lut(|value| {
        let normalized = ((value - in_black) / range)
            .clamp(0.0, 1.0)
            .powf(1.0 / gamma);
        out_black + normalized * (out_white - out_black)
    })
}

fn curve_lut(data: &CurveData) -> Option<Lut> {
    match data {
        CurveData::Map(map) => Some(lut(|value| {
            let position = value * 255.0;
            let low = position.floor() as usize;
            let high = (low + 1).min(255);
            let a = f32::from(map[low.min(255)]);
            let b = f32::from(map[high]);
            (a + (b - a) * (position - low as f32)) / 255.0
        })),
        CurveData::Points(points) if points.len() >= 2 => {
            let spline = Spline::from_points(
                points
                    .iter()
                    .map(|&(output, input)| (f64::from(input) / 255.0, f64::from(output) / 255.0)),
            );
            Some(lut(|value| spline.evaluate(f64::from(value)) as f32))
        }
        CurveData::Points(_) => None,
    }
}

pub(crate) struct ChannelLuts {
    master: Option<Lut>,
    channels: [Option<Lut>; 4],
}

impl ChannelLuts {
    fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let mut out = rgb;
        for (channel, value) in out.iter_mut().enumerate() {
            *value = self.channel(channel, *value);
        }
        out
    }

    fn channel(&self, channel: usize, value: f32) -> f32 {
        let mut value = value;
        if let Some(table) = &self.channels[channel] {
            value = sample(table, value);
        }
        if let Some(table) = &self.master {
            value = sample(table, value);
        }
        value
    }
}

fn levels(levels: &Levels) -> ChannelLuts {
    let identity = rikka_psd::LevelRecord::default();
    let pick = |index: usize| {
        let record = levels.records.get(index)?;
        (*record != identity).then(|| level_lut(record))
    };
    ChannelLuts {
        master: pick(0),
        channels: [pick(1), pick(2), pick(3), pick(4)],
    }
}

fn curves(curves: &Curves) -> ChannelLuts {
    let mut luts = ChannelLuts {
        master: None,
        channels: [None, None, None, None],
    };
    for (channel, data) in curves.channel_curves() {
        let table = curve_lut(data);
        match channel {
            0 => luts.master = table,
            1..=4 => luts.channels[usize::from(channel) - 1] = table,
            _ => {}
        }
    }
    luts
}

fn hue_mask(hue: f32, range: [i16; 4]) -> f32 {
    let wrap = |value: f32| value.rem_euclid(360.0);
    let [a, b, c, d] = range.map(f32::from);
    let centered = wrap(hue - a);
    let left = wrap(b - a);
    let center = wrap(c - a);
    let right = wrap(d - a);
    if centered >= left && centered <= center {
        1.0
    } else if centered < left {
        if left > 0.0 { centered / left } else { 0.0 }
    } else if centered <= right {
        if right > center {
            (right - centered) / (right - center)
        } else {
            0.0
        }
    } else {
        0.0
    }
}

fn lightness(value: f32, amount: f32) -> f32 {
    if amount > 0.0 {
        value * (1.0 - amount) + amount
    } else {
        value * (1.0 + amount)
    }
}

fn saturate(value: f32, amount: f32) -> f32 {
    if amount > 0.0 {
        (value / (1.0 - amount + 0.01)).clamp(0.0, 1.0)
    } else {
        (value * (1.0 + amount)).clamp(0.0, 1.0)
    }
}

fn hue_saturation(settings: &HueSaturation, rgb: [f32; 3]) -> [f32; 3] {
    if settings.colorize {
        let [hue, saturation, light] = settings.colorization.map(f32::from);
        let base = rgb_to_hsl(rgb.map(f64::from));
        let l = lightness(base[2] as f32, light / 100.0);
        return hsl_to_rgb([f64::from(hue), f64::from(saturation / 100.0), f64::from(l)])
            .map(|value| value as f32);
    }
    let [master_hue, master_saturation, master_lightness] = settings.master.map(f32::from);
    let light = rgb.map(|value| lightness(value, master_lightness / 100.0));
    let hsl = rgb_to_hsl(light.map(f64::from));
    let base_hue = hsl[0] as f32;
    let (mut hue_shift, mut saturation_shift, mut lightness_shift) = (0.0f32, 0.0f32, 0.0f32);
    for (range, values) in &settings.ranges {
        let weight = if hsl[1] > 0.0 {
            hue_mask(base_hue, *range)
        } else {
            0.0
        };
        if weight <= 0.0 {
            continue;
        }
        hue_shift += f32::from(values[0]) * weight;
        saturation_shift += f32::from(values[1]) / 100.0 * weight;
        lightness_shift += f32::from(values[2]) / 100.0 * weight;
    }
    let hue = (base_hue + hue_shift + master_hue).rem_euclid(360.0);
    let saturation = saturate(
        saturate(hsl[1] as f32, master_saturation / 100.0),
        saturation_shift.clamp(-1.0, 1.0),
    );
    let mut out =
        hsl_to_rgb([f64::from(hue), f64::from(saturation), hsl[2]]).map(|value| value as f32);
    if lightness_shift != 0.0 {
        out = out.map(|value| lightness(value, lightness_shift.clamp(-1.0, 1.0) * saturation));
    }
    out
}

fn color_balance(
    shadows: &[i16; 3],
    midtones: &[i16; 3],
    highlights: &[i16; 3],
    preserve: bool,
    rgb: [f32; 3],
) -> [f32; 3] {
    let (a, b, scale) = (0.25f32, 0.333f32, 0.7f32);
    let shadow_weight = |v: f32| ((v - b) / -a + 0.5).clamp(0.0, 1.0) * scale;
    let midtone_weight = |v: f32| {
        ((v - b) / a + 0.5).clamp(0.0, 1.0) * ((v + b - 1.0) / -a + 0.5).clamp(0.0, 1.0) * scale
    };
    let highlight_weight = |v: f32| ((v + b - 1.0) / a + 0.5).clamp(0.0, 1.0) * scale;
    let mut out = rgb;
    for (channel, value) in out.iter_mut().enumerate() {
        let original = *value;
        *value = (original
            + f32::from(shadows[channel]) / 100.0 * shadow_weight(original)
            + f32::from(midtones[channel]) / 100.0 * midtone_weight(original)
            + f32::from(highlights[channel]) / 100.0 * highlight_weight(original))
        .clamp(0.0, 1.0);
    }
    if preserve {
        let original = rgb_to_hsl(rgb.map(f64::from));
        let changed = rgb_to_hsl(out.map(f64::from));
        out = hsl_to_rgb([changed[0], changed[1], original[2]]).map(|value| value as f32);
    }
    out
}

fn black_white(settings: BlackWhite) -> impl Fn([f32; 3]) -> [f32; 3] + 'static {
    let tint = settings
        .tint
        .map(|tint| tint.to_rgb().map(|value| value as f32));
    move |rgb| black_white_gray(&settings, tint, rgb)
}

fn black_white_gray(settings: &BlackWhite, tint: Option<[f32; 3]>, rgb: [f32; 3]) -> [f32; 3] {
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| rgb[a].total_cmp(&rgb[b]));
    let (min, mid, max) = (rgb[order[0]], rgb[order[1]], rgb[order[2]]);
    let primary = [settings.reds, settings.greens, settings.blues][order[2]] as f32 / 100.0;
    let secondary_weight = match (order[2], order[1]) {
        (0, 1) | (1, 0) => settings.yellows,
        (1, 2) | (2, 1) => settings.cyans,
        _ => settings.magentas,
    } as f32
        / 100.0;
    let gray = (min + (mid - min) * secondary_weight + (max - mid) * primary).clamp(0.0, 1.0);
    match tint {
        Some(tint) => set_lum(tint, gray),
        None => [gray; 3],
    }
}

fn photo_filter(filter: &PhotoFilter) -> impl Fn([f32; 3]) -> [f32; 3] + 'static {
    let tint = match filter.color {
        PhotoFilterColor::Color(color) => color.to_rgb().map(|value| value as f32),
        PhotoFilterColor::Lab(values) => lab_to_rgb(
            f64::from(values[0]) / 100.0,
            f64::from(values[1]) / 100.0,
            f64::from(values[2]) / 100.0,
        )
        .map(|value| value as f32),
    };
    let density = filter.density as f32 / 100.0;
    let preserve_luminosity = filter.preserve_luminosity;
    move |rgb| {
        let filtered = [0, 1, 2]
            .map(|channel| rgb[channel] + (rgb[channel] * tint[channel] - rgb[channel]) * density);
        if preserve_luminosity {
            set_lum(filtered, lum(rgb))
        } else {
            filtered
        }
    }
}

fn channel_mixer(monochrome: bool, records: &[[i16; 5]], rgb: [f32; 3]) -> [f32; 3] {
    let mix = |record: &[i16; 5]| {
        (rgb[0] * f32::from(record[0])
            + rgb[1] * f32::from(record[1])
            + rgb[2] * f32::from(record[2]))
            / 100.0
            + f32::from(record[4]) / 100.0
    };
    if monochrome {
        let record = records.get(3).or_else(|| records.first());
        return record.map_or(rgb, |record| [mix(record).clamp(0.0, 1.0); 3]);
    }
    let mut out = rgb;
    for (channel, value) in out.iter_mut().enumerate() {
        if let Some(record) = records.get(channel) {
            *value = mix(record).clamp(0.0, 1.0);
        }
    }
    out
}

fn vibrance(settings: &Vibrance, rgb: [f32; 3]) -> [f32; 3] {
    let hsl = rgb_to_hsl(rgb.map(f64::from));
    let saturation = hsl[1] as f32;
    let vibrance = settings.vibrance as f32 / 100.0;
    let weight = (1.0 - saturation).powi(2);
    let adjusted = saturate(
        saturate(saturation, settings.saturation as f32 / 100.0),
        vibrance * weight,
    );
    hsl_to_rgb([hsl[0], f64::from(adjusted), hsl[2]]).map(|value| value as f32)
}

fn selective_color(absolute: bool, plates: &[[i16; 4]; 10], rgb: [f32; 3]) -> [f32; 3] {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    let mid = rgb[0] + rgb[1] + rgb[2] - max - min;
    let mut weights = [0.0f32; 10];
    let channel_max = [rgb[0] == max, rgb[1] == max, rgb[2] == max];
    let channel_min = [rgb[0] == min, rgb[1] == min, rgb[2] == min];
    if channel_max[0] {
        weights[1] = max - mid;
    }
    if channel_min[2] {
        weights[2] = mid - min;
    }
    if channel_max[1] {
        weights[3] = max - mid;
    }
    if channel_min[0] {
        weights[4] = mid - min;
    }
    if channel_max[2] {
        weights[5] = max - mid;
    }
    if channel_min[1] {
        weights[6] = mid - min;
    }
    weights[7] = ((min - 0.5) * 2.0).max(0.0);
    weights[9] = ((0.5 - max) * 2.0).max(0.0);
    weights[8] = (1.0 - (max - 0.5).abs() - (min - 0.5).abs()).max(0.0);
    let mut ink = rgb.map(|value| 1.0 - value);
    let black = 1.0 - max;
    for (plate, weight) in plates.iter().zip(weights).skip(1) {
        if weight <= 0.0 {
            continue;
        }
        for channel in 0..3 {
            let adjust = f32::from(plate[channel]) / 100.0;
            let black_adjust = f32::from(plate[3]) / 100.0;
            let base = if absolute { 1.0 } else { ink[channel] };
            let black_base = if absolute { 1.0 } else { black };
            ink[channel] += weight * (adjust * base + black_adjust * black_base);
        }
    }
    ink.map(|value| (1.0 - value).clamp(0.0, 1.0))
}

fn gradient_map(map: &GradientMap) -> impl Fn([f32; 3]) -> [f32; 3] + 'static {
    let mut stops: Vec<(f32, f32, [f32; 3])> = map
        .color_stops
        .iter()
        .map(|stop| {
            (
                stop.location as f32 / 4096.0,
                stop.midpoint as f32 / 100.0,
                stop.color.to_rgb().map(|value| value as f32),
            )
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let reversed = map.reversed;
    move |rgb| sample_gradient_map(&stops, reversed, rgb)
}

fn sample_gradient_map(stops: &[(f32, f32, [f32; 3])], reversed: bool, rgb: [f32; 3]) -> [f32; 3] {
    let mut t = lum(rgb).clamp(0.0, 1.0);
    if reversed {
        t = 1.0 - t;
    }
    let Some(first) = stops.first() else {
        return rgb;
    };
    if t <= first.0 {
        return first.2;
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t <= b.0 {
            let span = (b.0 - a.0).max(1e-6);
            let local = (t - a.0) / span;
            let midpoint = a.1.clamp(0.01, 0.99);
            let eased = if local < midpoint {
                0.5 * local / midpoint
            } else {
                0.5 + 0.5 * (local - midpoint) / (1.0 - midpoint)
            };
            return [0, 1, 2].map(|channel| a.2[channel] + (b.2[channel] - a.2[channel]) * eased);
        }
    }
    stops.last().map_or(rgb, |stop| stop.2)
}

pub(crate) struct Cube {
    size: usize,
    table: Vec<[f32; 3]>,
    domain: ([f32; 3], [f32; 3]),
}

impl Cube {
    pub(crate) fn parse(lookup: &ColorLookup) -> Option<Cube> {
        let text = std::str::from_utf8(&lookup.data).ok()?;
        match lookup.format {
            LutFormat::Cube => Self::parse_cube(text),
            LutFormat::ThreeDl => Self::parse_3dl(text),
            LutFormat::Look | LutFormat::Other => {
                Self::parse_cube(text).or_else(|| Self::parse_3dl(text))
            }
        }
    }

    fn parse_cube(text: &str) -> Option<Cube> {
        let mut size = 0usize;
        let mut domain = ([0.0f32; 3], [1.0f32; 3]);
        let mut table = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("TITLE") {
                continue;
            }
            let mut parts = line.split_whitespace();
            let first = parts.next()?;
            match first {
                "LUT_3D_SIZE" => size = parts.next()?.parse().ok()?,
                "LUT_1D_SIZE" => return None,
                "DOMAIN_MIN" => {
                    domain.0 = [
                        parts.next()?.parse().ok()?,
                        parts.next()?.parse().ok()?,
                        parts.next()?.parse().ok()?,
                    ]
                }
                "DOMAIN_MAX" => {
                    domain.1 = [
                        parts.next()?.parse().ok()?,
                        parts.next()?.parse().ok()?,
                        parts.next()?.parse().ok()?,
                    ]
                }
                _ => {
                    let r: f32 = first.parse().ok()?;
                    let g: f32 = parts.next()?.parse().ok()?;
                    let b: f32 = parts.next()?.parse().ok()?;
                    table.push([r, g, b]);
                }
            }
        }
        (size >= 2 && table.len() == size * size * size).then_some(Cube {
            size,
            table,
            domain,
        })
    }

    fn parse_3dl(text: &str) -> Option<Cube> {
        let mut rows: Vec<Vec<f32>> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                line.split_whitespace()
                    .filter_map(|value| value.parse().ok())
                    .collect()
            })
            .collect();
        let header = rows.iter().position(|row| row.len() > 3)?;
        let size = rows[header].len();
        let max_input = rows[header].last().copied().unwrap_or(1.0).max(1.0);
        rows.drain(..=header);
        let entries: Vec<[f32; 3]> = rows
            .iter()
            .filter(|row| row.len() == 3)
            .map(|row| [row[0], row[1], row[2]])
            .collect();
        if entries.len() != size * size * size {
            return None;
        }
        let peak = entries.iter().flatten().fold(1.0f32, |a, &b| a.max(b));
        let bits = if peak > 4095.0 {
            65535.0
        } else if peak > 1023.0 {
            4095.0
        } else {
            max_input.max(peak)
        };
        let mut table = vec![[0.0f32; 3]; entries.len()];
        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let source = entries[(r * size + g) * size + b];
                    table[(b * size + g) * size + r] = source.map(|value| value / bits);
                }
            }
        }
        Some(Cube {
            size,
            table,
            domain: ([0.0; 3], [1.0; 3]),
        })
    }

    pub(crate) fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size - 1;
        let position: [f32; 3] = [0, 1, 2].map(|channel| {
            let span = (self.domain.1[channel] - self.domain.0[channel]).max(1e-6);
            ((rgb[channel] - self.domain.0[channel]) / span).clamp(0.0, 1.0) * n as f32
        });
        let low = position.map(|value| (value.floor() as usize).min(n));
        let high = low.map(|value| (value + 1).min(n));
        let fraction = [0, 1, 2].map(|channel| position[channel] - low[channel] as f32);
        let at = |r: usize, g: usize, b: usize| self.table[(b * self.size + g) * self.size + r];
        let mut out = [0.0f32; 3];
        for corner in 0..8usize {
            let r = if corner & 1 != 0 { high[0] } else { low[0] };
            let g = if corner & 2 != 0 { high[1] } else { low[1] };
            let b = if corner & 4 != 0 { high[2] } else { low[2] };
            let weight = (if corner & 1 != 0 {
                fraction[0]
            } else {
                1.0 - fraction[0]
            }) * (if corner & 2 != 0 {
                fraction[1]
            } else {
                1.0 - fraction[1]
            }) * (if corner & 4 != 0 {
                fraction[2]
            } else {
                1.0 - fraction[2]
            });
            let value = at(r, g, b);
            for channel in 0..3 {
                out[channel] += value[channel] * weight;
            }
        }
        out.map(|value| value.clamp(0.0, 1.0))
    }
}

fn brightness_contrast(brightness: f32, contrast: f32, legacy: bool) -> Lut {
    let b = brightness / 150.0;
    let c = f64::from(contrast / 100.0);
    if legacy {
        let shift = brightness / 255.0;
        let slope = ((contrast / 100.0 + 1.0) * std::f32::consts::FRAC_PI_4)
            .tan()
            .clamp(0.0, 255.0);
        return lut(|value| ((value + shift - 0.5) * slope + 0.5).clamp(0.0, 1.0));
    }
    let xs = [0.0, 63.0 / 255.0, 191.0 / 255.0, 1.0];
    let delta = 25.0 / 255.0;
    let ys = [0.0, xs[1] - c * delta, xs[2] + c * delta, 1.0];
    let contrast_curve = Spline::new(&xs, &ys);
    let b = f64::from(b);
    let magnitude = b.abs();
    let term = |a: f64, x: f64, r: f64| a * x.powf(r);
    let samples: Vec<(f64, f64)> = (0..LUT)
        .map(|index| {
            let t = index as f64 / (LUT - 1) as f64;
            let h = 0.5
                * (magnitude * (term(1.65, t, 0.35) + term(-1.0, t, 10.0))
                    + (1.0 - magnitude) * (term(1.96, t, 0.4) + term(1.0, t, 4.0))
                    + term(1.0, t, 1.25));
            let shift = b * t * (1.0 - t) * h;
            (t - shift, contrast_curve.evaluate(t) + shift)
        })
        .collect();
    lut(|value| {
        let value = f64::from(value);
        let upper = samples
            .partition_point(|sample| sample.0 < value)
            .clamp(1, samples.len() - 1);
        let (a, b) = (samples[upper - 1], samples[upper]);
        let span = b.0 - a.0;
        let fraction = if span.abs() < 1e-12 {
            0.0
        } else {
            ((value - a.0) / span).clamp(0.0, 1.0)
        };
        (a.1 + (b.1 - a.1) * fraction) as f32
    })
}

pub(crate) enum Operation {
    Luts(ChannelLuts),
    Pixel(Box<dyn Fn([f32; 3]) -> [f32; 3]>),
    Invert,
}

impl Operation {
    pub(crate) fn from_layer(layer: &rikka_psd::Layer) -> Option<Operation> {
        let adjustment = layer.adjustment()?;
        let generator = layer.descriptor(&[b"CgEd"]);
        Some(match adjustment {
            AdjustmentLayer::Binary(binary) => match binary {
                Adjustment::BrightnessContrast {
                    brightness,
                    contrast,
                    ..
                } => {
                    let legacy = generator
                        .is_none_or(|descriptor| descriptor.bool("useLegacy").unwrap_or(false));
                    let brightness = generator
                        .and_then(|descriptor| descriptor.number("Brgh"))
                        .map_or(f32::from(brightness), |value| value as f32);
                    let contrast = generator
                        .and_then(|descriptor| descriptor.number("Cntr"))
                        .map_or(f32::from(contrast), |value| value as f32);
                    let table = brightness_contrast(brightness, contrast, legacy);
                    Operation::Luts(ChannelLuts {
                        master: Some(table),
                        channels: [None, None, None, None],
                    })
                }
                Adjustment::Levels(settings) => Operation::Luts(levels(&settings)),
                Adjustment::Curves(settings) => Operation::Luts(curves(&settings)),
                Adjustment::Exposure {
                    exposure,
                    offset,
                    gamma,
                    ..
                } => {
                    let gamma = gamma.clamp(0.01, 9.99);
                    Operation::Luts(ChannelLuts {
                        master: Some(lut(|value| {
                            let linear = value.powf(2.2) * exposure.exp2() + offset;
                            linear.clamp(0.0, 1.0).powf(1.0 / gamma).powf(1.0 / 2.2)
                        })),
                        channels: [None, None, None, None],
                    })
                }
                Adjustment::HueSaturation(settings) => {
                    Operation::Pixel(Box::new(move |rgb| hue_saturation(&settings, rgb)))
                }
                Adjustment::ColorBalance {
                    shadows,
                    midtones,
                    highlights,
                    preserve_luminosity,
                } => Operation::Pixel(Box::new(move |rgb| {
                    color_balance(&shadows, &midtones, &highlights, preserve_luminosity, rgb)
                })),
                Adjustment::PhotoFilter(filter) => {
                    Operation::Pixel(Box::new(photo_filter(&filter)))
                }
                Adjustment::ChannelMixer {
                    monochrome,
                    records,
                    ..
                } => Operation::Pixel(Box::new(move |rgb| {
                    channel_mixer(monochrome, &records, rgb)
                })),
                Adjustment::Invert => Operation::Invert,
                Adjustment::Posterize { levels } => {
                    let levels = f32::from(levels.clamp(2, 255));
                    Operation::Luts(ChannelLuts {
                        master: Some(lut(|value| {
                            (value * levels * 0.999_99).floor() / (levels - 1.0)
                        })),
                        channels: [None, None, None, None],
                    })
                }
                Adjustment::Threshold { level } => {
                    let level = f32::from(level) / 255.0;
                    Operation::Pixel(Box::new(move |rgb| {
                        [if lum(rgb) >= level { 1.0 } else { 0.0 }; 3]
                    }))
                }
                Adjustment::GradientMap(map) => Operation::Pixel(Box::new(gradient_map(&map))),
                Adjustment::SelectiveColor {
                    absolute, plates, ..
                } => Operation::Pixel(Box::new(move |rgb| selective_color(absolute, &plates, rgb))),
            },
            AdjustmentLayer::Vibrance(settings) => {
                Operation::Pixel(Box::new(move |rgb| vibrance(&settings, rgb)))
            }
            AdjustmentLayer::BlackWhite(settings) => {
                Operation::Pixel(Box::new(black_white(settings)))
            }
            AdjustmentLayer::ColorLookup(lookup) => {
                let cube = Cube::parse(&lookup)?;
                Operation::Pixel(Box::new(move |rgb| cube.apply(rgb)))
            }
        })
    }
}

use std::f64::consts::{FRAC_PI_2, SQRT_2};

use crate::curve_profile::CurveProfile;

pub(crate) const PRO_SQUARE_R: f32 = 1.0e4;
pub(crate) const PRO_CIRCLE_R: f32 = 2.0;
pub(crate) const PRO_LINE_R: f32 = 1.0;
pub(crate) const PRO_SQUARE_IN_R: f32 = 0.0;

const CHORD_TOLERANCE: f64 = 1.0e-13;
const CHORD_ITERATIONS: usize = 10;
const SMOOTH_ITERATIONS: usize = 10;
const SMOOTH_TOLERANCE: f64 = 1.0e-7;
const CIRCLE_FULLNESS: [f32; 11] = [
    0.0, 0.559, 0.642, 0.551, 0.646, 0.624, 0.646, 0.619, 0.647, 0.639, 0.647,
];

#[derive(Clone, Debug, Default)]
pub(crate) struct Samples {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Spacing {
    pub(crate) main: Samples,
    pub(crate) power: Option<Samples>,
}

impl Spacing {
    pub(crate) fn power_samples(&self) -> &Samples {
        self.power.as_ref().unwrap_or(&self.main)
    }
}

pub(crate) fn power_segments(segments: usize) -> usize {
    segments.next_power_of_two().max(4)
}

pub(crate) fn superellipse_spacing(segments: usize, super_r: f32) -> Spacing {
    if segments <= 1 {
        return Spacing::default();
    }
    let power = power_segments(segments);
    Spacing {
        main: even_superellipse_chords(segments, super_r),
        power: (power != segments).then(|| even_superellipse_chords(power, super_r)),
    }
}

pub(crate) fn custom_spacing(segments: usize, profile: &CurveProfile) -> Spacing {
    if segments <= 1 {
        return Spacing::default();
    }
    let power = power_segments(segments);
    let swapped = |count: usize| {
        let points = profile.sample(count);
        Samples {
            x: points.iter().map(|point| f64::from(point.y)).collect(),
            y: points.iter().map(|point| f64::from(point.x)).collect(),
        }
    };
    Spacing {
        power: (power != segments).then(|| swapped(power)),
        main: swapped(segments),
    }
}

pub(crate) fn superellipse_co(x: f64, r: f32, rbig: bool) -> f64 {
    let r = f64::from(r);
    if rbig {
        (1.0 - x.powf(r)).powf(1.0 / r)
    } else {
        1.0 - (1.0 - (1.0 - x).powf(r)).powf(1.0 / r)
    }
}

fn chord_error(x: f64, y: f64, x0: f64, y0: f64, target: f64) -> f64 {
    ((x - x0).powi(2) + (y - y0).powi(2)).sqrt() - target
}

fn superellipse_chord_endpoint(x0: f64, target: f64, r: f32, rbig: bool) -> f64 {
    let y0 = superellipse_co(x0, r, rbig);
    let mut xmin = (x0 + SQRT_2 / 2.0 * target).min(1.0);
    let mut xmax = (x0 + target).min(1.0);
    let ymin = superellipse_co(xmin, r, rbig);
    let ymax = superellipse_co(xmax, r, rbig);
    let mut dmaxerr = chord_error(xmax, ymax, x0, y0, target);
    let mut dminerr = chord_error(xmin, ymin, x0, y0, target);
    let mut xnew = xmax - dmaxerr * (xmax - xmin) / (dmaxerr - dminerr);
    let mut last_upper = true;
    for _ in 0..CHORD_ITERATIONS {
        let ynew = superellipse_co(xnew, r, rbig);
        let dnewerr = chord_error(xnew, ynew, x0, y0, target);
        if dnewerr.abs() < CHORD_TOLERANCE {
            break;
        }
        if dnewerr < 0.0 {
            xmin = xnew;
            dminerr = dnewerr;
            xnew = match last_upper {
                false => (dmaxerr / 2.0 * xmin - dminerr * xmax) / (dmaxerr / 2.0 - dminerr),
                true => xmax - dmaxerr * (xmax - xmin) / (dmaxerr - dminerr),
            };
            last_upper = false;
        } else {
            xmax = xnew;
            dmaxerr = dnewerr;
            xnew = match last_upper {
                true => (dmaxerr * xmin - dminerr / 2.0 * xmax) / (dmaxerr - dminerr / 2.0),
                false => xmax - dmaxerr * (xmax - xmin) / (dmaxerr - dminerr),
            };
            last_upper = true;
        }
    }
    xnew
}

fn even_superellipse_chords_general(segments: usize, r: f32) -> Samples {
    let mut x = vec![0.0f64; segments + 1];
    let mut y = vec![0.0f64; segments + 1];
    let imax = segments.div_ceil(2) - 1;
    let odd = segments % 2 == 1;
    let (rbig, mx) = match r > 1.0 {
        true => (true, 0.5f64.powf(1.0 / f64::from(r))),
        false => (false, 1.0 - 0.5f64.powf(1.0 / f64::from(r))),
    };
    for i in 0..=imax {
        x[i] = i as f64 * mx / segments as f64 * 2.0;
        y[i] = superellipse_co(x[i], r, rbig);
    }
    y[0] = 1.0;
    for _ in 0..SMOOTH_ITERATIONS {
        let mut sum = 0.0;
        let mut dmin = 2.0f64;
        let mut dmax = 0.0f64;
        for i in 0..imax {
            let d = ((x[i + 1] - x[i]).powi(2) + (y[i + 1] - y[i]).powi(2)).sqrt();
            sum += d;
            dmax = dmax.max(d);
            dmin = dmin.min(d);
        }
        let average = match odd {
            true => {
                sum += SQRT_2 / 2.0 * (y[imax] - x[imax]);
                sum / (imax as f64 + 0.5)
            }
            false => {
                sum += ((x[imax] - mx).powi(2) + (y[imax] - mx).powi(2)).sqrt();
                sum / (imax as f64 + 1.0)
            }
        };
        let precise = dmax - average <= SMOOTH_TOLERANCE && dmin - average >= SMOOTH_TOLERANCE;
        if precise {
            break;
        }
        for i in 1..=imax {
            x[i] = superellipse_chord_endpoint(x[i - 1], average, r, rbig);
            y[i] = superellipse_co(x[i], r, rbig);
        }
    }
    if !odd {
        x[imax + 1] = mx;
        y[imax + 1] = mx;
    }
    for i in imax + 1..=segments {
        y[i] = x[segments - i];
        x[i] = y[segments - i];
    }
    if !rbig {
        for i in 0..=segments {
            let previous_x = x[i];
            x[i] = 1.0 - y[i];
            y[i] = 1.0 - previous_x;
        }
    }
    Samples { x, y }
}

pub(crate) fn even_superellipse_chords(segments: usize, r: f32) -> Samples {
    let n = segments as f64;
    let half = segments / 2;
    let odd = segments % 2 == 1;
    let mut x = vec![0.0f64; segments + 1];
    let mut y = vec![0.0f64; segments + 1];
    if r == PRO_LINE_R {
        for i in 0..=segments {
            x[i] = i as f64 / n;
            y[i] = 1.0 - i as f64 / n;
        }
    } else if r == PRO_CIRCLE_R {
        let step = FRAC_PI_2 / n;
        for i in 0..=segments {
            x[i] = (i as f64 * step).sin();
            y[i] = (i as f64 * step).cos();
        }
    } else if r == PRO_SQUARE_IN_R || r == PRO_SQUARE_R {
        let step = match odd {
            false => 1.0 / half as f64,
            true => 1.0 / (half as f64 + SQRT_2 / 2.0),
        };
        for i in 0..=half {
            if r == PRO_SQUARE_IN_R {
                x[i] = 0.0;
                y[i] = 1.0 - i as f64 * step;
            } else {
                x[i] = i as f64 * step;
                y[i] = 1.0;
            }
            x[segments - i] = y[i];
            y[segments - i] = x[i];
        }
    } else {
        return even_superellipse_chords_general(segments, r);
    }
    Samples { x, y }
}

pub(crate) fn superellipse_fullness(segments: usize, profile: f32, super_r: f32) -> f32 {
    if super_r == PRO_LINE_R {
        0.0
    } else if super_r == PRO_CIRCLE_R && (1..=CIRCLE_FULLNESS.len()).contains(&segments) {
        CIRCLE_FULLNESS[segments - 1]
    } else if segments.is_multiple_of(2) {
        2.4506 * profile - 0.000_003 * segments as f32 - 0.6266
    } else {
        2.3635 * profile + 0.000_152 * segments as f32 - 0.6060
    }
}

pub(crate) fn custom_fullness(segments: usize, samples: &Samples) -> f32 {
    let mut fullness = 0.0f32;
    for i in 0..segments {
        fullness += (samples.x[i] + samples.y[i]) as f32 / (2.0 * segments as f32);
    }
    fullness
}

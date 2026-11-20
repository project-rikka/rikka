use rikka_types::srgb;

use crate::pixels::Depth;

pub fn hsb_to_rgb(hue_degrees: f64, saturation: f64, brightness: f64) -> [f64; 3] {
    let h = hue_degrees.rem_euclid(360.0) / 60.0;
    let c = brightness * saturation;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = brightness - c;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

pub fn rgb_to_hsb(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    [hue, saturation, max]
}

pub fn rgb_to_hsl(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    if delta == 0.0 {
        return [0.0, 0.0, lightness];
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs()).max(1e-12);
    let hue = if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    [hue, saturation.min(1.0), lightness]
}

pub fn hsl_to_rgb(hsl: [f64; 3]) -> [f64; 3] {
    let [hue, saturation, lightness] = hsl;
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let h = hue.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = lightness - c / 2.0;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

pub fn cmyk_to_rgb(c: f64, m: f64, y: f64, k: f64) -> [f64; 3] {
    [
        (1.0 - c) * (1.0 - k),
        (1.0 - m) * (1.0 - k),
        (1.0 - y) * (1.0 - k),
    ]
}

pub fn rgb_to_cmyk(rgb: [f64; 3]) -> [f64; 4] {
    let [r, g, b] = rgb;
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let scale = 1.0 - k;
    [
        (1.0 - r - k) / scale,
        (1.0 - g - k) / scale,
        (1.0 - b - k) / scale,
        k,
    ]
}

const D50: [f64; 3] = [0.964_22, 1.0, 0.825_21];

const XYZ_D50_TO_SRGB: [[f64; 3]; 3] = [
    [3.134_136, -1.617_386, -0.490_662],
    [-0.978_795, 1.916_254, 0.033_443],
    [0.071_955, -0.228_977, 1.405_386],
];

const SRGB_TO_XYZ_D50: [[f64; 3]; 3] = [
    [0.436_065_7, 0.385_151_5, 0.143_078_8],
    [0.222_493_2, 0.716_887_9, 0.060_618_9],
    [0.013_923_2, 0.097_081_0, 0.714_185_2],
];

fn multiply(matrix: &[[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * vector[0] + matrix[0][1] * vector[1] + matrix[0][2] * vector[2],
        matrix[1][0] * vector[0] + matrix[1][1] * vector[1] + matrix[1][2] * vector[2],
        matrix[2][0] * vector[0] + matrix[2][1] * vector[1] + matrix[2][2] * vector[2],
    ]
}

pub fn lab_to_rgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let inverse = |t: f64| {
        if t > 6.0 / 29.0 {
            t * t * t
        } else {
            3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0)
        }
    };
    let xyz = [
        D50[0] * inverse(fx),
        D50[1] * inverse(fy),
        D50[2] * inverse(fz),
    ];
    let linear = multiply(&XYZ_D50_TO_SRGB, xyz);
    linear.map(|value| srgb::from_linear_f64(value.clamp(0.0, 1.0)))
}

pub fn rgb_to_lab(rgb: [f64; 3]) -> [f64; 3] {
    let linear = rgb.map(|value| srgb::to_linear_f64(value.clamp(0.0, 1.0)));
    let xyz = multiply(&SRGB_TO_XYZ_D50, linear);
    let forward = |t: f64| {
        if t > (6.0f64 / 29.0).powi(3) {
            t.cbrt()
        } else {
            t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0
        }
    };
    let fx = forward(xyz[0] / D50[0]);
    let fy = forward(xyz[1] / D50[1]);
    let fz = forward(xyz[2] / D50[2]);
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn luminosity(rgb: [f64; 3]) -> f64 {
    0.3 * rgb[0] + 0.59 * rgb[1] + 0.11 * rgb[2]
}

pub fn rgb_to_gray(rgb: [f32; 3]) -> f32 {
    0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]
}

pub fn lab_encoded([l, a, b]: [f64; 3], depth: Depth) -> [f64; 3] {
    let (scale, zero) = lab_coding(depth);
    [l / 100.0, (a + zero) / scale, (b + zero) / scale]
}

pub fn lab_decoded(encoded: [f32; 3], depth: Depth) -> [f64; 3] {
    let (scale, zero) = lab_coding(depth);
    [
        f64::from(encoded[0]) * 100.0,
        f64::from(encoded[1]) * scale - zero,
        f64::from(encoded[2]) * scale - zero,
    ]
}

fn lab_coding(depth: Depth) -> (f64, f64) {
    match depth {
        Depth::Sixteen => (65535.0 / 256.0, 32768.0 / 256.0),
        _ => (255.0, 128.0),
    }
}

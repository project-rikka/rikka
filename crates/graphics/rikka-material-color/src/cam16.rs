use std::f64::consts::PI;
use std::sync::LazyLock;

use crate::color::{Rgb, WHITE_POINT_D65, y_from_lstar};
use crate::math::{lerp, signum};

const BACKGROUND_LSTAR: f64 = 50.0;
const SURROUND: f64 = 2.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewingConditions {
    pub(crate) background_y_to_white_point_y: f64,
    pub(crate) aw: f64,
    pub(crate) nbb: f64,
    pub(crate) ncb: f64,
    pub(crate) c: f64,
    pub(crate) n_c: f64,
    pub(crate) rgb_d: [f64; 3],
    pub(crate) fl: f64,
    pub(crate) z: f64,
}

pub(crate) static STANDARD: LazyLock<ViewingConditions> = LazyLock::new(|| {
    let white_point = WHITE_POINT_D65;
    let adapting_luminance = 200.0 / PI * y_from_lstar(50.0) / 100.0;
    let xyz = white_point;
    let (r_w, g_w, b_w) = (
        xyz[2].mul_add(-0.051461, xyz[0].mul_add(0.401288, xyz[1] * 0.650173)),
        xyz[2].mul_add(0.045854, xyz[0].mul_add(-0.250268, xyz[1] * 1.204414)),
        xyz[2].mul_add(0.953127, xyz[0].mul_add(-0.002079, xyz[1] * 0.048952)),
    );
    let f = 0.8 + (SURROUND / 10.0);
    let c = if f >= 0.9 {
        lerp(0.59, 0.69, (f - 0.9) * 10.0)
    } else {
        lerp(0.525, 0.59, (f - 0.8) * 10.0)
    };
    let d = f * (1.0f64 / 3.6f64).mul_add(-((-adapting_luminance - 42.0) / 92.0).exp(), 1.0);
    let d = d.clamp(0.0, 1.0);
    let rgb_d = [
        d.mul_add(100.0 / r_w, 1.0) - d,
        d.mul_add(100.0 / g_w, 1.0) - d,
        d.mul_add(100.0 / b_w, 1.0) - d,
    ];
    let k = 1.0 / 5.0f64.mul_add(adapting_luminance, 1.0);
    let k4 = k * k * k * k;
    let k4_f = 1.0 - k4;
    let fl = k4.mul_add(
        adapting_luminance,
        0.1 * k4_f * k4_f * (5.0 * adapting_luminance).cbrt(),
    );
    let n = y_from_lstar(BACKGROUND_LSTAR) / white_point[1];
    let z = 1.48 + n.sqrt();
    let nbb = 0.725 / n.powf(0.2);
    let rgb_a_factors = [
        (fl * rgb_d[0] * r_w / 100.0).powf(0.42),
        (fl * rgb_d[1] * g_w / 100.0).powf(0.42),
        (fl * rgb_d[2] * b_w / 100.0).powf(0.42),
    ];
    let rgb_a = rgb_a_factors.map(|factor| (400.0 * factor) / (factor + 27.13));
    let aw = (40.0f64.mul_add(rgb_a[0], 20.0 * rgb_a[1]) + rgb_a[2]) / 20.0 * nbb;
    ViewingConditions {
        background_y_to_white_point_y: n,
        aw,
        nbb,
        ncb: nbb,
        c,
        n_c: f,
        rgb_d,
        fl,
        z,
    }
});

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cam16 {
    pub(crate) hue: f64,
    pub(crate) chroma: f64,
}

impl Cam16 {
    pub(crate) fn of(rgb: Rgb) -> Cam16 {
        let conditions = &*STANDARD;
        let [x, y, z] = rgb.xyz();
        let (r_c, g_c, b_c) = (
            0.051461f64.mul_add(-z, 0.401288f64.mul_add(x, 0.650173 * y)),
            0.045854f64.mul_add(z, (-0.250268f64).mul_add(x, 1.204414 * y)),
            0.953127f64.mul_add(z, (-0.002079f64).mul_add(x, 0.048952 * y)),
        );
        let r_d = conditions.rgb_d[0] * r_c;
        let g_d = conditions.rgb_d[1] * g_c;
        let b_d = conditions.rgb_d[2] * b_c;
        let (r_af, g_af, b_af) = (
            (conditions.fl * r_d.abs() / 100.0).powf(0.42),
            (conditions.fl * g_d.abs() / 100.0).powf(0.42),
            (conditions.fl * b_d.abs() / 100.0).powf(0.42),
        );
        let r_a = signum(r_d) * 400.0 * r_af / (r_af + 27.13);
        let g_a = signum(g_d) * 400.0 * g_af / (g_af + 27.13);
        let b_a = signum(b_d) * 400.0 * b_af / (b_af + 27.13);
        let (a, b, u, p2) = (
            (11.0f64.mul_add(r_a, -12.0 * g_a) + b_a) / 11.0,
            2.0f64.mul_add(-b_a, r_a + g_a) / 9.0,
            21.0f64.mul_add(b_a, 20.0f64.mul_add(r_a, 20.0 * g_a)) / 20.0,
            (40.0f64.mul_add(r_a, 20.0 * g_a) + b_a) / 20.0,
        );
        let degrees = b.atan2(a).to_degrees();
        let hue = if degrees < 0.0 {
            degrees + 360.0
        } else if degrees >= 360.0 {
            degrees - 360.0
        } else {
            degrees
        };
        let ac = p2 * conditions.nbb;
        let j = 100.0 * (ac / conditions.aw).powf(conditions.c * conditions.z);
        let hue_prime = if hue < 20.14 { hue + 360.0 } else { hue };
        let e_hue = (1.0 / 4.0) * ((hue_prime.to_radians() + 2.0).cos() + 3.8);
        let p1 = 50000.0 / 13.0 * e_hue * conditions.n_c * conditions.ncb;
        let t = p1 * a.hypot(b) / (u + 0.305);
        let alpha = t.powf(0.9)
            * (1.64 - 0.29f64.powf(conditions.background_y_to_white_point_y)).powf(0.73);
        Cam16 {
            hue,
            chroma: alpha * (j / 100.0).sqrt(),
        }
    }
}

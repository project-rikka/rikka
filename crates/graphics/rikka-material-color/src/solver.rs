use std::f64::consts::PI;

use crate::cam16::STANDARD;
use crate::color::{Rgb, y_from_lstar};
use crate::critical_planes::CRITICAL_PLANES;
use crate::math::{matrix_multiply, sanitize_degrees, signum};

const SCALED_DISCOUNT_FROM_LINRGB: [[f64; 3]; 3] = [
    [
        0.001200833568784504,
        0.002389694492170889,
        0.0002795742885861124,
    ],
    [
        0.0005891086651375999,
        0.0029785502573438758,
        0.0003270666104008398,
    ],
    [
        0.00010146692491640572,
        0.0005364214359186694,
        0.0032979401770712076,
    ],
];
const LINRGB_FROM_SCALED_DISCOUNT: [[f64; 3]; 3] = [
    [1373.2198709594231, -1100.4251190754821, -7.278681089101213],
    [-271.815969077903, 559.6580465940733, -32.46047482791194],
    [1.9622899599665666, -57.173814538844006, 308.7233197812385],
];
const Y_FROM_LINRGB: [f64; 3] = [0.2126, 0.7152, 0.0722];
const OUTSIDE: [f64; 3] = [-1.0; 3];

fn sanitize_radians(angle: f64) -> f64 {
    PI.mul_add(8.0, angle) % (PI * 2.0)
}

fn true_delinearized(component: f64) -> f64 {
    let normalized = component / 100.0;
    let encoded = if normalized <= 0.0031308 {
        normalized * 12.92
    } else {
        1.055f64.mul_add(normalized.powf(1.0 / 2.4), -0.055)
    };
    encoded * 255.0
}

fn chromatic_adaptation(component: f64) -> f64 {
    let af = component.abs().powf(0.42);
    signum(component) * 400.0 * af / (af + 27.13)
}

fn hue_of(linrgb: [f64; 3]) -> f64 {
    let scaled_discount = matrix_multiply(linrgb, SCALED_DISCOUNT_FROM_LINRGB);
    let r_a = chromatic_adaptation(scaled_discount[0]);
    let g_a = chromatic_adaptation(scaled_discount[1]);
    let b_a = chromatic_adaptation(scaled_discount[2]);
    let a = (11.0f64.mul_add(r_a, -12.0 * g_a) + b_a) / 11.0;
    let b = 2.0f64.mul_add(-b_a, r_a + g_a) / 9.0;
    b.atan2(a)
}

fn are_in_cyclic_order(a: f64, b: f64, c: f64) -> bool {
    sanitize_radians(b - a) < sanitize_radians(c - a)
}

fn set_coordinate(source: [f64; 3], coordinate: f64, target: [f64; 3], axis: usize) -> [f64; 3] {
    let t = (coordinate - source[axis]) / (target[axis] - source[axis]);
    [
        (target[0] - source[0]).mul_add(t, source[0]),
        (target[1] - source[1]).mul_add(t, source[1]),
        (target[2] - source[2]).mul_add(t, source[2]),
    ]
}

fn is_bounded(x: f64) -> bool {
    (0.0..=100.0).contains(&x)
}

fn nth_vertex(y: f64, n: i32) -> [f64; 3] {
    let [k_r, k_g, k_b] = Y_FROM_LINRGB;
    let coord_a: f64 = if n % 4 <= 1 { 0.0 } else { 100.0 };
    let coord_b: f64 = if n % 2 == 0 { 0.0 } else { 100.0 };
    let vertex = if n < 4 {
        let (g, b) = (coord_a, coord_b);
        let r = b.mul_add(-k_b, g.mul_add(-k_g, y)) / k_r;
        (r, [r, g, b])
    } else if n < 8 {
        let (b, r) = (coord_a, coord_b);
        let g = b.mul_add(-k_b, r.mul_add(-k_r, y)) / k_g;
        (g, [r, g, b])
    } else {
        let (r, g) = (coord_a, coord_b);
        let b = g.mul_add(-k_g, r.mul_add(-k_r, y)) / k_b;
        (b, [r, g, b])
    };
    match is_bounded(vertex.0) {
        true => vertex.1,
        false => OUTSIDE,
    }
}

fn bisect_to_segment(y: f64, target_hue: f64) -> [[f64; 3]; 2] {
    let mut left = OUTSIDE;
    let mut right = left;
    let mut left_hue = 0.0;
    let mut right_hue = 0.0;
    let mut initialized = false;
    let mut uncut = true;
    for n in 0..12 {
        let mid = nth_vertex(y, n);
        if mid[0] < 0.0 {
            continue;
        }
        let mid_hue = hue_of(mid);
        if !initialized {
            left = mid;
            right = mid;
            left_hue = mid_hue;
            right_hue = mid_hue;
            initialized = true;
            continue;
        }
        if uncut || are_in_cyclic_order(left_hue, mid_hue, right_hue) {
            uncut = false;
            if are_in_cyclic_order(left_hue, target_hue, mid_hue) {
                right = mid;
                right_hue = mid_hue;
            } else {
                left = mid;
                left_hue = mid_hue;
            }
        }
    }
    [left, right]
}

fn critical_plane_below(x: f64) -> i16 {
    (x - 0.5).floor() as i16
}

fn critical_plane_above(x: f64) -> i16 {
    (x - 0.5).ceil() as i16
}

fn bisect_to_limit(y: f64, target_hue: f64) -> [f64; 3] {
    let [mut left, mut right] = bisect_to_segment(y, target_hue);
    let mut left_hue = hue_of(left);
    for axis in 0..3 {
        if (left[axis] - right[axis]).abs() > f64::EPSILON {
            let [mut l_plane, mut r_plane] = if left[axis] < right[axis] {
                [
                    critical_plane_below(true_delinearized(left[axis])),
                    critical_plane_above(true_delinearized(right[axis])),
                ]
            } else {
                [
                    critical_plane_above(true_delinearized(left[axis])),
                    critical_plane_below(true_delinearized(right[axis])),
                ]
            };
            for _ in 0..8 {
                if (r_plane - l_plane).abs() <= 1 {
                    break;
                }
                let m_plane = ((f64::from(l_plane) + f64::from(r_plane)) / 2.0).floor() as i16;
                let coordinate = CRITICAL_PLANES[(m_plane.max(0) as usize).min(254)];
                let mid = set_coordinate(left, coordinate, right, axis);
                let mid_hue = hue_of(mid);
                if are_in_cyclic_order(left_hue, target_hue, mid_hue) {
                    right = mid;
                    r_plane = m_plane;
                } else {
                    left = mid;
                    left_hue = mid_hue;
                    l_plane = m_plane;
                }
            }
        }
    }
    [
        (left[0] + right[0]) / 2.0,
        (left[1] + right[1]) / 2.0,
        (left[2] + right[2]) / 2.0,
    ]
}

fn inverse_chromatic_adaptation(adapted: f64) -> f64 {
    let adapted_abs = adapted.abs();
    let base = (27.13 * adapted_abs / (400.0 - adapted_abs)).max(0.0);
    signum(adapted) * base.powf(1.0 / 0.42)
}

fn find_by_j(hue_radians: f64, chroma: f64, y: f64) -> Option<Rgb> {
    let mut j = y.sqrt() * 11.0;
    let conditions = &*STANDARD;
    let t_inner_coeff =
        1.0 / (1.64 - 0.29f64.powf(conditions.background_y_to_white_point_y)).powf(0.73);
    let e_hue = 0.25 * ((hue_radians + 2.0).cos() + 3.8);
    let p1 = e_hue * (50000.0 / 13.0) * conditions.n_c * conditions.ncb;
    let (h_sin, h_cos) = (hue_radians.sin(), hue_radians.cos());
    for round in 0..5 {
        let j_normalized = j / 100.0;
        let alpha = if chroma == 0.0 || j == 0.0 {
            0.0
        } else {
            chroma / j_normalized.sqrt()
        };
        let t = (alpha * t_inner_coeff).powf(1.0 / 0.9);
        let ac = conditions.aw * j_normalized.powf(1.0 / conditions.c / conditions.z);
        let p2 = ac / conditions.nbb;
        let gamma = 23.0 * (p2 + 0.305) * t
            / (108.0 * t).mul_add(h_sin, 23.0f64.mul_add(p1, 11.0 * t * h_cos));
        let a = gamma * h_cos;
        let b = gamma * h_sin;
        let (r_a, g_a, b_a) = (
            288.0f64.mul_add(b, 460.0f64.mul_add(p2, 451.0 * a)) / 1403.0,
            261.0f64.mul_add(-b, 460.0f64.mul_add(p2, -891.0 * a)) / 1403.0,
            6300.0f64.mul_add(-b, 460.0f64.mul_add(p2, -220.0 * a)) / 1403.0,
        );
        let scaled = [
            inverse_chromatic_adaptation(r_a),
            inverse_chromatic_adaptation(g_a),
            inverse_chromatic_adaptation(b_a),
        ];
        let linrgb = matrix_multiply(scaled, LINRGB_FROM_SCALED_DISCOUNT);
        if linrgb.iter().any(|&component| component < 0.0) {
            return None;
        }
        let [k_r, k_g, k_b] = Y_FROM_LINRGB;
        let fnj = k_b.mul_add(linrgb[2], k_r.mul_add(linrgb[0], k_g * linrgb[1]));
        if fnj <= 0.0 {
            return None;
        }
        if round == 4 || (fnj - y).abs() < 0.002 {
            if linrgb.iter().any(|&component| component > 100.01) {
                return None;
            }
            return Some(Rgb::from_linear(linrgb));
        }
        j -= (fnj - y) * j / (2.0 * fnj);
    }
    None
}

pub(crate) fn solve(hue_degrees: f64, chroma: f64, lstar: f64) -> Rgb {
    if chroma < 0.0001 || !(0.0001..=99.9999).contains(&lstar) {
        return Rgb::from_lstar(lstar);
    }
    let hue_radians = sanitize_degrees(hue_degrees).to_radians();
    let y = y_from_lstar(lstar);
    find_by_j(hue_radians, chroma, y)
        .unwrap_or_else(|| Rgb::from_linear(bisect_to_limit(y, hue_radians)))
}

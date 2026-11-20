use crate::math::matrix_multiply;

pub(crate) const SRGB_TO_XYZ: [[f64; 3]; 3] = [
    [0.41233895, 0.35762064, 0.18051042],
    [0.2126, 0.7152, 0.0722],
    [0.01932141, 0.11916382, 0.95034478],
];
const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [
        3.2413774792388685,
        -1.5376652402851851,
        -0.49885366846268053,
    ],
    [-0.9691452513005321, 1.8758853451067872, 0.04156585616912061],
    [
        0.05562093689691305,
        -0.20395524564742123,
        1.0571799111220335,
    ],
];
pub(crate) const WHITE_POINT_D65: [f64; 3] = [95.047, 100.0, 108.883];
const LAB_EPSILON: f64 = 216.0 / 24389.0;
const LAB_KAPPA: f64 = 24389.0 / 27.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lab {
    pub(crate) l: f64,
    pub(crate) a: f64,
    pub(crate) b: f64,
}

impl Rgb {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Rgb {
        Rgb { red, green, blue }
    }

    #[must_use]
    pub fn lstar(self) -> f64 {
        116.0f64.mul_add(lab_f(self.xyz()[1] / 100.0), -16.0)
    }

    pub(crate) fn linear(self) -> [f64; 3] {
        [
            linearized(self.red),
            linearized(self.green),
            linearized(self.blue),
        ]
    }

    pub(crate) fn xyz(self) -> [f64; 3] {
        matrix_multiply(self.linear(), SRGB_TO_XYZ)
    }

    pub(crate) fn from_linear(linear: [f64; 3]) -> Rgb {
        Rgb::new(
            delinearized(linear[0]),
            delinearized(linear[1]),
            delinearized(linear[2]),
        )
    }

    pub(crate) fn from_lstar(lstar: f64) -> Rgb {
        let component = delinearized(y_from_lstar(lstar));
        Rgb::new(component, component, component)
    }

    pub(crate) fn lab(self) -> Lab {
        let [red, green, blue] = self.linear();
        let matrix = SRGB_TO_XYZ;
        let x = matrix[0][2].mul_add(blue, matrix[0][0].mul_add(red, matrix[0][1] * green));
        let y = matrix[1][2].mul_add(blue, matrix[1][0].mul_add(red, matrix[1][1] * green));
        let z = matrix[2][2].mul_add(blue, matrix[2][0].mul_add(red, matrix[2][1] * green));
        let fx = lab_f(x / WHITE_POINT_D65[0]);
        let fy = lab_f(y / WHITE_POINT_D65[1]);
        let fz = lab_f(z / WHITE_POINT_D65[2]);
        Lab {
            l: 116.0f64.mul_add(fy, -16.0),
            a: 500.0 * (fx - fy),
            b: 200.0 * (fy - fz),
        }
    }

    pub(crate) fn from_lab(lab: Lab) -> Rgb {
        let fy = (lab.l + 16.0) / 116.0;
        let fx = lab.a / 500.0 + fy;
        let fz = fy - lab.b / 200.0;
        let x = lab_invf(fx) * WHITE_POINT_D65[0];
        let y = lab_invf(fy) * WHITE_POINT_D65[1];
        let z = lab_invf(fz) * WHITE_POINT_D65[2];
        let matrix = XYZ_TO_SRGB;
        Rgb::from_linear([
            matrix[0][2].mul_add(z, matrix[0][0].mul_add(x, matrix[0][1] * y)),
            matrix[1][2].mul_add(z, matrix[1][0].mul_add(x, matrix[1][1] * y)),
            matrix[2][2].mul_add(z, matrix[2][0].mul_add(x, matrix[2][1] * y)),
        ])
    }
}

pub(crate) fn y_from_lstar(lstar: f64) -> f64 {
    100.0 * lab_invf((lstar + 16.0) / 116.0)
}

pub(crate) fn lstar_from_y(y: f64) -> f64 {
    lab_f(y / 100.0).mul_add(116.0, -16.0)
}

fn linearized(component: u8) -> f64 {
    let normalized = f64::from(component) / 255.0;
    if normalized <= 0.040449936 {
        normalized / 12.92 * 100.0
    } else {
        ((normalized + 0.055) / 1.055).powf(2.4) * 100.0
    }
}

fn delinearized(component: f64) -> u8 {
    let normalized = component / 100.0;
    let encoded = if normalized <= 0.0031308 {
        normalized * 12.92
    } else {
        1.055f64.mul_add(normalized.powf(1.0 / 2.4), -0.055)
    };
    (encoded * 255.0).round() as u8
}

fn lab_f(t: f64) -> f64 {
    if t > LAB_EPSILON {
        t.cbrt()
    } else {
        LAB_KAPPA.mul_add(t, 16.0) / 116.0
    }
}

fn lab_invf(ft: f64) -> f64 {
    let cubed = ft * ft * ft;
    if cubed > LAB_EPSILON {
        cubed
    } else {
        116.0f64.mul_add(ft, -16.0) / LAB_KAPPA
    }
}

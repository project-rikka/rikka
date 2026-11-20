use std::f64::consts::PI;

use glam::{DMat3, DVec3};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Lobe {
    pub frame: DMat3,
    pub m11: f64,
    pub m22: f64,
    pub m13: f64,
    pub magnitude: f64,
    pub fresnel: f64,
    matrix: DMat3,
    inverse: DMat3,
    determinant: f64,
}

impl Lobe {
    pub(crate) fn new() -> Self {
        let mut lobe = Self {
            frame: DMat3::IDENTITY,
            m11: 1.0,
            m22: 1.0,
            m13: 0.0,
            magnitude: 1.0,
            fresnel: 0.0,
            matrix: DMat3::IDENTITY,
            inverse: DMat3::IDENTITY,
            determinant: 1.0,
        };
        lobe.update();
        lobe
    }

    pub(crate) fn update(&mut self) {
        let shape = DMat3::from_cols(
            DVec3::new(self.m11, 0.0, 0.0),
            DVec3::new(0.0, self.m22, 0.0),
            DVec3::new(self.m13, 0.0, 1.0),
        );
        self.matrix = self.frame * shape;
        self.inverse = self.matrix.inverse();
        self.determinant = self.matrix.determinant().abs();
    }

    pub(crate) fn matrix(&self) -> DMat3 {
        self.matrix
    }

    pub(crate) fn evaluate(&self, light: DVec3) -> f64 {
        let original = (self.inverse * light).normalize();
        let transformed = self.matrix * original;
        let length = transformed.length();
        let jacobian = self.determinant / (length * length * length);
        self.magnitude * original.z.max(0.0) / PI / jacobian
    }

    pub(crate) fn sample(&self, u1: f64, u2: f64) -> DVec3 {
        let theta = u1.sqrt().acos();
        let phi = 2.0 * PI * u2;
        let local = DVec3::new(
            theta.sin() * phi.cos(),
            theta.sin() * phi.sin(),
            theta.cos(),
        );
        (self.matrix * local).normalize()
    }
}

use std::ops::Mul;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3(pub [[f64; 3]; 3]);

impl Matrix3 {
    pub const IDENTITY: Self = Self([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    pub fn from_columns(columns: [[f64; 3]; 3]) -> Self {
        Self(std::array::from_fn(|row| {
            std::array::from_fn(|column| columns[column][row])
        }))
    }

    pub fn diagonal(values: [f64; 3]) -> Self {
        let [a, b, c] = values;
        Self([[a, 0.0, 0.0], [0.0, b, 0.0], [0.0, 0.0, c]])
    }

    pub fn rows(&self) -> [[f64; 3]; 3] {
        self.0
    }

    pub fn apply(&self, vector: [f64; 3]) -> [f64; 3] {
        self.0
            .map(|row| row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2])
    }

    pub fn determinant(&self) -> f64 {
        let m = &self.0;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    pub fn inverse(&self) -> Option<Self> {
        let determinant = self.determinant();
        if determinant.abs() < f64::EPSILON {
            return None;
        }
        let m = &self.0;
        let cofactor = |r0: usize, r1: usize, c0: usize, c1: usize| {
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        let adjugate = [
            [
                cofactor(1, 2, 1, 2),
                -cofactor(0, 2, 1, 2),
                cofactor(0, 1, 1, 2),
            ],
            [
                -cofactor(1, 2, 0, 2),
                cofactor(0, 2, 0, 2),
                -cofactor(0, 1, 0, 2),
            ],
            [
                cofactor(1, 2, 0, 1),
                -cofactor(0, 2, 0, 1),
                cofactor(0, 1, 0, 1),
            ],
        ];
        Some(Self(
            adjugate.map(|row| row.map(|value| value / determinant)),
        ))
    }

    pub fn is_identity(&self, tolerance: f64) -> bool {
        self.0
            .iter()
            .zip(Self::IDENTITY.0.iter())
            .all(|(row, unit)| {
                row.iter()
                    .zip(unit.iter())
                    .all(|(value, expected)| (value - expected).abs() <= tolerance)
            })
    }
}

impl Mul for Matrix3 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self(std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                (0..3)
                    .map(|inner| self.0[row][inner] * other.0[inner][column])
                    .sum()
            })
        }))
    }
}

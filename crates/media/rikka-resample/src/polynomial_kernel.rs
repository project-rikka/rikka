use crate::cursor::Cursor;
use crate::kernel::Kernel;
use crate::lagrange;
use crate::planes::Planes;
use crate::polynomial_degree::PolynomialDegree;

pub(crate) struct PolynomialKernel {
    degree: PolynomialDegree,
}

impl PolynomialKernel {
    pub(crate) fn new(degree: PolynomialDegree) -> PolynomialKernel {
        PolynomialKernel { degree }
    }

    fn weights(&self, frac: f64) -> [f32; 4] {
        match self.degree {
            PolynomialDegree::Nearest => [1.0, 0.0, 0.0, 0.0],
            PolynomialDegree::Cubic => lagrange::cubic(frac),
        }
    }
}

impl Kernel for PolynomialKernel {
    fn lookbehind(&self) -> usize {
        self.degree.lookbehind()
    }

    fn lookahead(&self) -> usize {
        self.degree.points() - 1 - self.degree.lookbehind()
    }

    fn render(&mut self, planes: &Planes, cursor: &mut Cursor, end: usize, output: &mut Vec<f32>) {
        let points = self.degree.points();
        while cursor.index() < end {
            let weights = self.weights(cursor.frac());
            let start = cursor.index() - self.lookbehind();
            for plane in planes.as_slice() {
                let samples = &plane[start..start + points];
                output.push(weights.iter().zip(samples).map(|(w, s)| w * s).sum());
            }
            cursor.advance();
        }
    }
}

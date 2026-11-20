#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PolynomialDegree {
    Nearest,
    Cubic,
}

impl PolynomialDegree {
    pub(crate) fn points(self) -> usize {
        match self {
            PolynomialDegree::Nearest => 1,
            PolynomialDegree::Cubic => 4,
        }
    }

    pub(crate) fn lookbehind(self) -> usize {
        match self {
            PolynomialDegree::Nearest => 0,
            PolynomialDegree::Cubic => 1,
        }
    }
}

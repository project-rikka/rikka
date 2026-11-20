use crate::polynomial_degree::PolynomialDegree;
use crate::sinc_params::SincParams;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ResampleAlgorithm {
    Sinc(SincParams),
    Polynomial(PolynomialDegree),
}

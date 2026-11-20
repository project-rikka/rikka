use crate::algorithm::ResampleAlgorithm;
use crate::polynomial_degree::PolynomialDegree;
use crate::sinc_params::SincParams;
use crate::window::WindowFunction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ResampleQuality {
    Fast,
    #[default]
    Balanced,
    High,
    Ultra,
}

impl ResampleQuality {
    pub(crate) fn algorithm(self) -> ResampleAlgorithm {
        match self {
            ResampleQuality::Fast => ResampleAlgorithm::Polynomial(PolynomialDegree::Cubic),
            ResampleQuality::Balanced => ResampleAlgorithm::Sinc(
                SincParams::new(128, WindowFunction::BlackmanHarris).oversampling(128),
            ),
            ResampleQuality::High => ResampleAlgorithm::Sinc(
                SincParams::new(256, WindowFunction::BlackmanHarris2).oversampling(256),
            ),
            ResampleQuality::Ultra => ResampleAlgorithm::Sinc(
                SincParams::new(512, WindowFunction::BlackmanHarris2).oversampling(256),
            ),
        }
    }
}

impl From<ResampleQuality> for ResampleAlgorithm {
    fn from(quality: ResampleQuality) -> ResampleAlgorithm {
        quality.algorithm()
    }
}

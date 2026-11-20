use crate::algorithm::ResampleAlgorithm;
use crate::cursor::Cursor;
use crate::planes::Planes;
use crate::polynomial_kernel::PolynomialKernel;
use crate::sinc_kernel::SincKernel;

pub(crate) trait Kernel {
    fn lookbehind(&self) -> usize;

    fn lookahead(&self) -> usize;

    fn render(&mut self, planes: &Planes, cursor: &mut Cursor, end: usize, output: &mut Vec<f32>);
}

pub(crate) enum AnyKernel {
    Sinc(SincKernel),
    Polynomial(PolynomialKernel),
}

impl AnyKernel {
    pub(crate) fn new(algorithm: ResampleAlgorithm, cursor: &Cursor) -> AnyKernel {
        match algorithm {
            ResampleAlgorithm::Sinc(params) => AnyKernel::Sinc(SincKernel::new(params, cursor)),
            ResampleAlgorithm::Polynomial(degree) => {
                AnyKernel::Polynomial(PolynomialKernel::new(degree))
            }
        }
    }
}

impl Kernel for AnyKernel {
    #[inline]
    fn lookbehind(&self) -> usize {
        match self {
            AnyKernel::Sinc(kernel) => kernel.lookbehind(),
            AnyKernel::Polynomial(kernel) => kernel.lookbehind(),
        }
    }

    #[inline]
    fn lookahead(&self) -> usize {
        match self {
            AnyKernel::Sinc(kernel) => kernel.lookahead(),
            AnyKernel::Polynomial(kernel) => kernel.lookahead(),
        }
    }

    #[inline]
    fn render(&mut self, planes: &Planes, cursor: &mut Cursor, end: usize, output: &mut Vec<f32>) {
        match self {
            AnyKernel::Sinc(kernel) => kernel.render(planes, cursor, end, output),
            AnyKernel::Polynomial(kernel) => kernel.render(planes, cursor, end, output),
        }
    }
}

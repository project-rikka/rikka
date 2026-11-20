use rikka_image_resample::Kernel;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    Bilinear,
    #[default]
    Lanczos3,
}

impl Filter {
    pub(crate) const fn kernel(self) -> Kernel {
        match self {
            Filter::Bilinear => Kernel::Bilinear,
            Filter::Lanczos3 => Kernel::Lanczos3,
        }
    }
}

use crate::shapers::complex_shaper::ComplexShaper;

#[derive(Debug)]
pub(crate) struct DefaultShaper;

pub(crate) static DEFAULT_SHAPER: DefaultShaper = DefaultShaper;

impl ComplexShaper for DefaultShaper {
    fn is_script_neutral(&self) -> bool {
        true
    }
}

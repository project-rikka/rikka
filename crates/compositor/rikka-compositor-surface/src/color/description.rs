use std::fmt;
use std::rc::Rc;

use rikka_colorimetry::{Description, Mastering};
use rikka_compositor_protocols::wp_color_manager_v1::{
    Primaries as NamedPrimaries, TransferFunction,
};

use super::named;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transfer {
    Named(TransferFunction),
    Power(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parametric {
    pub transfer: Transfer,
    pub named_primaries: Option<NamedPrimaries>,
    pub description: Description,
}

impl Parametric {
    pub fn named(primaries: NamedPrimaries, transfer: TransferFunction) -> Option<Self> {
        let signalled = Transfer::Named(transfer);
        let chromaticities = named::primaries(primaries);
        let luminance = named::default_luminance(signalled);
        Some(Self {
            transfer: signalled,
            named_primaries: Some(primaries),
            description: Description {
                primaries: chromaticities,
                transfer: named::function(signalled)?,
                luminance,
                mastering: Some(Mastering {
                    primaries: chromaticities,
                    min: luminance.min,
                    max: luminance.max,
                }),
                max_cll: None,
                max_fall: None,
            },
        })
    }
}

#[derive(Clone)]
pub struct ImageDescription(Rc<Described>);

struct Described {
    identity: u64,
    parametric: Parametric,
}

impl ImageDescription {
    pub fn identity(&self) -> u64 {
        self.0.identity
    }

    pub fn parametric(&self) -> &Parametric {
        &self.0.parametric
    }

    pub fn description(&self) -> &Description {
        &self.0.parametric.description
    }
}

impl PartialEq for ImageDescription {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for ImageDescription {}

impl fmt::Debug for ImageDescription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageDescription")
            .field("identity", &self.0.identity)
            .field("parametric", &self.0.parametric)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct Descriptions {
    next: u64,
}

impl Default for Descriptions {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl Descriptions {
    pub(crate) fn describe(&mut self, parametric: Parametric) -> ImageDescription {
        let identity = self.next;
        self.next += 1;
        ImageDescription(Rc::new(Described {
            identity,
            parametric,
        }))
    }
}

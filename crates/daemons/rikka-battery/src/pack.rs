use crate::device::DeviceId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Technology {
    #[default]
    Unknown,
    LithiumIon,
    LithiumPolymer,
    LithiumIronPhosphate,
    LeadAcid,
    NickelCadmium,
    NickelMetalHydride,
}

impl Technology {
    pub(crate) fn from_code(code: u32) -> Technology {
        match code {
            1 => Technology::LithiumIon,
            2 => Technology::LithiumPolymer,
            3 => Technology::LithiumIronPhosphate,
            4 => Technology::LeadAcid,
            5 => Technology::NickelCadmium,
            6 => Technology::NickelMetalHydride,
            _ => Technology::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Threshold {
    pub enabled: bool,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pack {
    pub id: DeviceId,
    pub name: String,
    pub technology: Technology,
    pub health: Option<f32>,
    pub energy: Option<f32>,
    pub energy_full: Option<f32>,
    pub energy_full_design: Option<f32>,
    pub voltage: Option<f32>,
    pub cycles: Option<u32>,
    pub threshold: Option<Threshold>,
}

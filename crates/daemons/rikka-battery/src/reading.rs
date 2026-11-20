use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Charge {
    #[default]
    Unknown,
    Charging,
    Discharging,
    Empty,
    Full,
}

impl Charge {
    pub(crate) fn from_code(code: u32) -> Charge {
        match code {
            1 | 5 => Charge::Charging,
            2 | 6 => Charge::Discharging,
            3 => Charge::Empty,
            4 => Charge::Full,
            _ => Charge::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Warning {
    #[default]
    None,
    Low,
    Critical,
    Action,
}

impl Warning {
    pub(crate) fn from_code(code: u32) -> Warning {
        match code {
            3 => Warning::Low,
            4 => Warning::Critical,
            5 => Warning::Action,
            _ => Warning::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub charge: f32,
    pub state: Charge,
    pub left: Option<Duration>,
    pub warning: Warning,
    pub rate: Option<f32>,
}

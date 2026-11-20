#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

impl Vendor {
    pub(crate) fn from_pci(id: u16) -> Self {
        match id {
            0x10de => Self::Nvidia,
            0x1002 | 0x1022 => Self::Amd,
            0x8086 => Self::Intel,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Memory {
    pub used: u64,
    pub total: u64,
}

#[derive(Clone, Debug)]
pub struct Adapter {
    pub name: String,
    pub driver: String,
    pub slot: String,
    pub usage: f32,
    pub encode: Option<f32>,
    pub decode: Option<f32>,
    pub dedicated: Option<Memory>,
    pub shared: Option<Memory>,
    pub temperature: Option<f32>,
    pub power: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MacAddress {
    #[default]
    Unset,
    Permanent,
    Preserve,
    Random,
    Stable,
    Fixed(String),
}

impl MacAddress {
    pub(crate) fn read(value: &str) -> Self {
        match value {
            "" => Self::Unset,
            "permanent" => Self::Permanent,
            "preserve" => Self::Preserve,
            "random" => Self::Random,
            "stable" => Self::Stable,
            address => Self::Fixed(address.to_owned()),
        }
    }

    pub(crate) fn write(&self) -> &str {
        match self {
            Self::Unset => "",
            Self::Permanent => "permanent",
            Self::Preserve => "preserve",
            Self::Random => "random",
            Self::Stable => "stable",
            Self::Fixed(address) => address,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxyMethod {
    #[default]
    None,
    Script,
}

impl ProxyMethod {
    pub(crate) fn read(name: &str) -> Self {
        match name {
            "auto" => Self::Script,
            _ => Self::None,
        }
    }

    pub(crate) fn write(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Script => "auto",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyProfile {
    pub method: ProxyMethod,
    pub script_url: String,
    pub browser_only: bool,
}

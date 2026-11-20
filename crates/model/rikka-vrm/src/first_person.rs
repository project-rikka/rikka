use rikka_model_core::NodeId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FirstPersonKind {
    #[default]
    Auto,
    Both,
    ThirdPersonOnly,
    FirstPersonOnly,
}

impl FirstPersonKind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Both => "both",
            Self::ThirdPersonOnly => "thirdPersonOnly",
            Self::FirstPersonOnly => "firstPersonOnly",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "auto" | "Auto" => Some(Self::Auto),
            "both" | "Both" => Some(Self::Both),
            "thirdPersonOnly" | "ThirdPersonOnly" => Some(Self::ThirdPersonOnly),
            "firstPersonOnly" | "FirstPersonOnly" => Some(Self::FirstPersonOnly),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshAnnotation {
    pub node: NodeId,
    pub kind: FirstPersonKind,
}

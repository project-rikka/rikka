use rikka_slang::ParameterSchema;
use rikka_spirv::Shader;
use serde::{Deserialize, Serialize};

use crate::effect::EffectLayout;
use crate::variant::Variant;

macro_rules! builtin_templates {
    ($($template:ident => ($id:literal, $module:literal),)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum BuiltinTemplate {
            $($template,)*
        }

        impl BuiltinTemplate {
            pub const COUNT: usize = [$($id),*].len();

            pub const ALL: [Self; Self::COUNT] = [$(Self::$template,)*];

            pub fn id(self) -> &'static str {
                match self {
                    $(Self::$template => $id,)*
                }
            }

            pub fn module(self) -> &'static str {
                match self {
                    $(Self::$template => $module,)*
                }
            }
        }
    };
}

builtin_templates! {
    Standard => ("rikka/standard", "rikka.templates.standard"),
    Wireframe => ("rikka/wireframe", "rikka.templates.wireframe"),
    Floor => ("rikka/floor", "rikka.templates.floor"),
    ShadowCatcher => ("rikka/shadow_catcher", "rikka.templates.shadow_catcher"),
    Water => ("rikka/water", "rikka.templates.water"),
    Skin => ("rikka/skin", "rikka.templates.skin"),
    Hair => ("rikka/hair", "rikka.templates.hair"),
    Eye => ("rikka/eye", "rikka.templates.eye"),
    Workbench => ("rikka/workbench", "rikka.templates.workbench"),
    Unlit => ("rikka/unlit", "rikka.templates.unlit"),
}

impl BuiltinTemplate {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|builtin| builtin.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateManifest {
    pub schema: ParameterSchema,
    pub deforms: bool,
    pub scatters: bool,
}

#[derive(Debug)]
pub struct TemplateArtifact {
    pub manifest: &'static str,
    pub variants: [Shader; Variant::COUNT],
}

impl TemplateArtifact {
    pub fn manifest(&self) -> TemplateManifest {
        serde_json::from_str(self.manifest)
            .expect("built-in template manifests are generated valid")
    }
}

#[derive(Debug)]
pub struct EffectArtifact {
    pub program: &'static str,
    pub entry: &'static str,
    pub spirv: Shader,
    pub layout: &'static str,
}

impl EffectArtifact {
    pub fn layout(&self) -> EffectLayout {
        serde_json::from_str(self.layout).expect("built-in effect layouts are generated valid")
    }
}

macro_rules! variants {
    ($($variant:ident => $entry:literal,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Variant {
            $($variant,)*
        }

        impl Variant {
            pub const COUNT: usize = [$($entry),*].len();

            pub const ALL: [Self; Self::COUNT] = [$(Self::$variant,)*];

            pub fn entry(self) -> &'static str {
                match self {
                    $(Self::$variant => $entry,)*
                }
            }
        }
    };
}

variants! {
    Forward => "rikka_entry_forward",
    HullVertex => "rikka_entry_hull_vertex",
    Hull => "rikka_entry_hull",
    Depth => "rikka_entry_depth",
    Cutout => "rikka_entry_cutout",
    Surfaces => "rikka_entry_surfaces",
    ForwardVertex => "rikka_entry_forward_vertex",
    DepthVertex => "rikka_entry_depth_vertex",
    SurfacesVertex => "rikka_entry_surfaces_vertex",
    SurfacesLayers => "rikka_entry_surfaces_layers",
    Tint => "rikka_entry_tint",
    Scatter => "rikka_entry_scatter",
    Hook => "rikka_entry_hook",
    ForwardEmissive => "rikka_entry_forward_emissive",
    ForwardTraced => "rikka_entry_forward_traced",
    HullTraced => "rikka_entry_hull_traced",
    ForwardEmissiveTraced => "rikka_entry_forward_emissive_traced",
}

impl Variant {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn traces_rays(self) -> bool {
        matches!(
            self,
            Self::ForwardTraced | Self::HullTraced | Self::ForwardEmissiveTraced
        )
    }

    fn shades_lights(self) -> bool {
        matches!(
            self,
            Self::Forward
                | Self::Hull
                | Self::ForwardEmissive
                | Self::ForwardTraced
                | Self::HullTraced
                | Self::ForwardEmissiveTraced
                | Self::Scatter
        )
    }

    pub(crate) fn by_cost() -> impl Iterator<Item = Self> {
        let shading = Self::ALL
            .into_iter()
            .filter(|variant| variant.shades_lights());
        shading.chain(
            Self::ALL
                .into_iter()
                .filter(|variant| !variant.shades_lights()),
        )
    }
}

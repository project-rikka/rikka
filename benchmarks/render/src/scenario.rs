use std::time::Duration;

use rikka_renderer::{OffscreenTarget, Renderer};
use rikka_renderer_3d::Result;
use rikka_renderer_3d::{Antialiasing, DynamicScale, RenderScale, Tier, Upscale};

use crate::allocations::Phase;
use crate::cast::Cast;
use crate::client::Client;
use crate::desktop::{Change, Desktop, Layout, SCREEN};
use crate::figures::Figures;
use crate::studio::{EXTENT, FULL_HD, Studio, Surfacing};

const GROWTH_PER_FRAME: u32 = 4;

const GROWTH_FRAMES: usize = 120;

const MULTISAMPLED: Antialiasing = Antialiasing::Multisample;

const UPSCALED: Antialiasing = Antialiasing::Upscale(Upscale::fixed(0.67));

const STEPPING: Antialiasing = Antialiasing::Upscale(Upscale {
    scale: RenderScale::Dynamic(DynamicScale {
        target: Duration::from_nanos(1),
        min: 0.25,
        max: DynamicScale::MAX,
    }),
    sharpness: Upscale::DEFAULT_SHARPNESS,
});

const MIKU: &str = "crates/model/rikka-mmd/assets/mmd/miku.pmx";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Mtoon,
    Anime,
    Genshin,
    Hybrid,
    Skin,
}

impl Style {
    fn preset(self) -> &'static str {
        match self {
            Style::Mtoon => "mtoon",
            Style::Anime => "anime",
            Style::Genshin => "genshin_body",
            Style::Hybrid => "hybrid",
            Style::Skin => "pbr_skin",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Spinner,
    Scroll,
    LiveResize,
    DesktopSpinner,
    DesktopVideo,
    Recompose,
    RecomposeReblurred,
    DragLone,
    Look,
    LookEffects,
    LookFigures,
    LookLights,
    LookTemporal,
    LookUpscaled,
    LookFullHd,
    LookEnvironment,
    LookEmitters,
    Traced,
    TracedTemporal,
    TracedUpscaled,
    TracedFullHd,
    TracedStepping,
    Fox,
    Knight,
    Miku,
    MikuStyled(Style),
    Skin,
    SkinFullHd,
    Glass,
    GlassFullHd,
    EffectPack,
    EffectPackUpscaled,
}

impl Scenario {
    pub const ALL: [Scenario; 36] = [
        Scenario::Spinner,
        Scenario::Scroll,
        Scenario::LiveResize,
        Scenario::DesktopSpinner,
        Scenario::DesktopVideo,
        Scenario::Recompose,
        Scenario::RecomposeReblurred,
        Scenario::DragLone,
        Scenario::Look,
        Scenario::LookEffects,
        Scenario::LookFigures,
        Scenario::LookLights,
        Scenario::LookTemporal,
        Scenario::LookUpscaled,
        Scenario::LookFullHd,
        Scenario::LookEnvironment,
        Scenario::LookEmitters,
        Scenario::Traced,
        Scenario::TracedTemporal,
        Scenario::TracedUpscaled,
        Scenario::TracedFullHd,
        Scenario::TracedStepping,
        Scenario::Fox,
        Scenario::Knight,
        Scenario::Miku,
        Scenario::MikuStyled(Style::Mtoon),
        Scenario::MikuStyled(Style::Anime),
        Scenario::MikuStyled(Style::Genshin),
        Scenario::MikuStyled(Style::Hybrid),
        Scenario::MikuStyled(Style::Skin),
        Scenario::Skin,
        Scenario::SkinFullHd,
        Scenario::Glass,
        Scenario::GlassFullHd,
        Scenario::EffectPack,
        Scenario::EffectPackUpscaled,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Scenario::Spinner => "client: spinner in a text window",
            Scenario::Scroll => "client: scroll a text window",
            Scenario::LiveResize => "client: live resize",
            Scenario::DesktopSpinner => "desktop: spinner over liquid glass",
            Scenario::DesktopVideo => "desktop: video under a liquid glass menu",
            Scenario::Recompose => "desktop: whole screen, blurs kept",
            Scenario::RecomposeReblurred => "desktop: whole screen, blurs remade",
            Scenario::DragLone => "desktop: drag a lone glass window",
            Scenario::Look => "three: look frame",
            Scenario::LookEffects => "three: look frame with effects",
            Scenario::LookFigures => "three: look frame with toon figures",
            Scenario::LookLights => "three: look frame with many and area lights",
            Scenario::LookTemporal => "three: look frame, temporal antialiasing",
            Scenario::LookUpscaled => "three: look frame, upscaled from 67%",
            Scenario::LookFullHd => "three: look frame at 1080p",
            Scenario::LookEnvironment => "three: look frame lit by an environment image",
            Scenario::LookEmitters => "three: look frame with 100k emitter particles at 1080p",
            Scenario::Traced => "three: ray traced look frame",
            Scenario::TracedTemporal => "three: ray traced look frame, temporal antialiasing",
            Scenario::TracedUpscaled => "three: ray traced look frame, upscaled from 67%",
            Scenario::TracedFullHd => "three: ray traced look frame at 1080p",
            Scenario::TracedStepping => {
                "three: ray traced look frame at 1080p, dynamic resolution stepping down"
            }
            Scenario::Fox => "three: fox",
            Scenario::Knight => "three: knight",
            Scenario::Miku => "three: miku",
            Scenario::MikuStyled(Style::Mtoon) => "three: miku styled mtoon",
            Scenario::MikuStyled(Style::Anime) => "three: miku styled anime",
            Scenario::MikuStyled(Style::Genshin) => "three: miku styled genshin_body",
            Scenario::MikuStyled(Style::Hybrid) => "three: miku styled hybrid",
            Scenario::MikuStyled(Style::Skin) => "three: miku styled pbr_skin",
            Scenario::Skin => "three: subsurface skin",
            Scenario::SkinFullHd => "three: subsurface skin at 1080p",
            Scenario::Glass => "three: refracting glass",
            Scenario::GlassFullHd => "three: refracting glass at 1080p",
            Scenario::EffectPack => "three: every built-in effect at 1080p",
            Scenario::EffectPackUpscaled => {
                "three: every built-in effect at 1080p, upscaled from 67%"
            }
        }
    }

    pub fn traces_rays(self) -> bool {
        matches!(
            self,
            Scenario::Traced
                | Scenario::TracedTemporal
                | Scenario::TracedUpscaled
                | Scenario::TracedFullHd
                | Scenario::TracedStepping
        )
    }

    pub(crate) fn open(self, renderer: &mut Renderer) -> Result<Running> {
        match self {
            Scenario::Spinner | Scenario::Scroll | Scenario::LiveResize => {
                let (width, height) = Client::extent();
                Ok(Running::Client {
                    client: Box::new(Client::new()),
                    target: renderer.create_target(width, height)?,
                    scenario: self,
                })
            }
            Scenario::Look => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, EXTENT, MULTISAMPLED)?),
            }),
            Scenario::LookEffects => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, EXTENT, MULTISAMPLED)?.effects(renderer)?),
            }),
            Scenario::LookTemporal => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, EXTENT, Antialiasing::Temporal)?),
            }),
            Scenario::LookUpscaled => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, EXTENT, UPSCALED)?),
            }),
            Scenario::EffectPack => Ok(Running::Studio {
                studio: Box::new(
                    Studio::new(renderer, FULL_HD, MULTISAMPLED)?.effect_pack(renderer)?,
                ),
            }),
            Scenario::EffectPackUpscaled => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, FULL_HD, UPSCALED)?.effect_pack(renderer)?),
            }),
            Scenario::LookFullHd => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, FULL_HD, MULTISAMPLED)?),
            }),
            Scenario::LookEnvironment => Ok(Running::Studio {
                studio: Box::new(
                    Studio::new(renderer, EXTENT, MULTISAMPLED)?.environment(renderer)?,
                ),
            }),
            Scenario::LookEmitters => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, FULL_HD, MULTISAMPLED)?.emitters()),
            }),
            Scenario::Traced => Ok(Running::Studio {
                studio: Box::new(
                    Studio::new(renderer, EXTENT, MULTISAMPLED)?.tiered(Tier::RayTraced),
                ),
            }),
            Scenario::TracedTemporal => Ok(Running::Studio {
                studio: Box::new(
                    Studio::new(renderer, EXTENT, Antialiasing::Temporal)?.tiered(Tier::RayTraced),
                ),
            }),
            Scenario::TracedUpscaled => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, EXTENT, UPSCALED)?.tiered(Tier::RayTraced)),
            }),
            Scenario::TracedFullHd => Ok(Running::Studio {
                studio: Box::new(
                    Studio::new(renderer, FULL_HD, MULTISAMPLED)?.tiered(Tier::RayTraced),
                ),
            }),
            Scenario::TracedStepping => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, FULL_HD, STEPPING)?.tiered(Tier::RayTraced)),
            }),
            Scenario::LookFigures => Ok(Running::Cast {
                cast: Box::new(Cast::new(renderer)?),
            }),
            Scenario::LookLights => Ok(Running::Studio {
                studio: Box::new(Studio::new(renderer, EXTENT, MULTISAMPLED)?.lights()),
            }),
            Scenario::Fox => figures(renderer, "crates/model/rikka-gltf/assets/fox.glb", None),
            Scenario::Knight => figures(renderer, "crates/model/rikka-fbx/assets/knight.fbx", None),
            Scenario::Miku => figures(renderer, MIKU, None),
            Scenario::MikuStyled(style) => figures(renderer, MIKU, Some(style.preset())),
            Scenario::Skin => surfaced(renderer, EXTENT, Surfacing::Skin),
            Scenario::SkinFullHd => surfaced(renderer, FULL_HD, Surfacing::Skin),
            Scenario::Glass => surfaced(renderer, EXTENT, Surfacing::Glass),
            Scenario::GlassFullHd => surfaced(renderer, FULL_HD, Surfacing::Glass),
            _ => {
                let (change, layout) = match self {
                    Scenario::DesktopSpinner => (Change::Spinner, Layout::Busy),
                    Scenario::DesktopVideo => (Change::VideoUnderMenu, Layout::Busy),
                    Scenario::Recompose => (Change::Everything { blurs_kept: true }, Layout::Busy),
                    Scenario::DragLone => (Change::Drag, Layout::Lone),
                    _ => (Change::Everything { blurs_kept: false }, Layout::Busy),
                };
                Ok(Running::Desktop {
                    desktop: Box::new(Desktop::new(renderer, layout)?),
                    target: renderer.create_target(SCREEN.0, SCREEN.1)?,
                    change,
                })
            }
        }
    }
}

pub(crate) enum Running {
    Client {
        client: Box<Client>,
        target: OffscreenTarget,
        scenario: Scenario,
    },
    Desktop {
        desktop: Box<Desktop>,
        target: OffscreenTarget,
        change: Change,
    },
    Studio {
        studio: Box<Studio>,
    },
    Figures {
        figures: Box<Figures>,
    },
    Cast {
        cast: Box<Cast>,
    },
}

fn figures(renderer: &mut Renderer, asset: &str, preset: Option<&str>) -> Result<Running> {
    Ok(Running::Figures {
        figures: Box::new(Figures::new(renderer, asset, preset)?),
    })
}

fn surfaced(renderer: &mut Renderer, extent: (u32, u32), surfacing: Surfacing) -> Result<Running> {
    Ok(Running::Studio {
        studio: Box::new(
            Studio::new(renderer, extent, MULTISAMPLED)?.surfaced(renderer, surfacing)?,
        ),
    })
}

impl Running {
    pub(crate) fn first(&mut self, renderer: &mut Renderer) -> Result<f32> {
        match self {
            Running::Client { client, target, .. } => Ok(client.draw(renderer, target)?),
            Running::Desktop {
                desktop, target, ..
            } => Ok(desktop.draw(renderer, target)?),
            Running::Studio { studio } => studio.draw(renderer, 0, Phase::Warming),
            Running::Figures { figures } => figures.draw(renderer, 0, Phase::Warming),
            Running::Cast { cast } => cast.draw(renderer, 0, Phase::Warming),
        }
    }

    pub(crate) fn change(&mut self, renderer: &mut Renderer, at: usize) -> Result<()> {
        match self {
            Running::Client {
                client, scenario, ..
            } => {
                match scenario {
                    Scenario::Scroll => client.scroll(at),
                    Scenario::LiveResize => {}
                    _ => client.spin(at),
                }
                Ok(())
            }
            Running::Desktop {
                desktop, change, ..
            } => Ok(desktop.change(renderer, *change, at)?),
            Running::Studio { .. } | Running::Figures { .. } | Running::Cast { .. } => Ok(()),
        }
    }

    pub(crate) fn allocated(&self) -> Option<u64> {
        match self {
            Running::Studio { studio } => Some(studio.allocated()),
            Running::Figures { figures } => Some(figures.allocated()),
            Running::Cast { cast } => Some(cast.allocated()),
            Running::Client { .. } | Running::Desktop { .. } => None,
        }
    }

    pub(crate) fn frame(
        &mut self,
        renderer: &mut Renderer,
        at: usize,
        phase: Phase,
    ) -> Result<f32> {
        match self {
            Running::Client {
                client,
                target,
                scenario,
            } => {
                if *scenario == Scenario::LiveResize {
                    let (width, height) = Client::extent();
                    let grown = GROWTH_PER_FRAME * (at % GROWTH_FRAMES) as u32;
                    *target = renderer.create_target(width + grown, height + grown / 2)?;
                }
                Ok(client.draw(renderer, target)?)
            }
            Running::Desktop {
                desktop, target, ..
            } => Ok(desktop.draw(renderer, target)?),
            Running::Studio { studio } => studio.draw(renderer, at + 1, phase),
            Running::Figures { figures } => figures.draw(renderer, at + 1, phase),
            Running::Cast { cast } => cast.draw(renderer, at + 1, phase),
        }
    }
}

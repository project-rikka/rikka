mod draw;
mod fx;
mod present;
mod renderer;
mod scene;
mod target;
mod text;
mod transform;

pub use fx::{
    BackdropCache, BackdropPass, BackdropShader, Blur, Joined, PassList, PostEffect, Spread,
};
pub use present::{Acquired, Buffering, Present, Sizing, SurfaceConfig};
pub use renderer::{
    AlphaMode, ColorId, Colors, CopiedDraw, CopySink, Damage, DamageSource, Destination, Extension,
    ExtensionSlot, ExternalDraw, ExternalImage, ExternalTarget, FrameStart, FrameWork, ImageColor,
    Preparer, Readback, Recorded, RenderOutput, Renderer, RendererParts, TRIM_AFTER, Textures,
};
#[cfg(unix)]
pub use renderer::{DmabufDraw, DmabufFence, DmabufFrame, DmabufOutput};
#[cfg(windows)]
pub use renderer::{SwapchainDraw, SwapchainFrame, SwapchainOutput};
#[cfg(windows)]
pub use rikka_gpu::{Acquire, Shown, Surface, SurfaceFit, Swapchain, SwapchainConfig};
#[cfg(unix)]
pub use rikka_gpu::{DmabufSpec, DmabufTarget, ExportedDmabuf, SharedRows};
pub use rikka_gpu::{Error, FrameTimeline, Gpu, Reclaimer, Result, TexelRect, TextureId, Uploads};
pub use rikka_types::Transform;
pub use scene::{
    Audience, GroupClip, GroupId, ImageSource, LayerId, Node, NodeEdit, NodeId, Scene, SceneFrame,
    Shape, Stroke, Viewer,
};
pub use target::{OffscreenTarget, TargetFormat};

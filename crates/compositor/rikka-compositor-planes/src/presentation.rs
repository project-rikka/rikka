use rikka_compositor_types::{ContentType, Flip, Pacing};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VrrPolicy {
    Never,
    #[default]
    Automatic,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presentation {
    pub vrr: bool,
    pub pacing: Pacing,
}

pub fn presentation(
    policy: VrrPolicy,
    fullscreen: Option<ContentType>,
    flip: Flip,
) -> Presentation {
    let vrr = match policy {
        VrrPolicy::Never => false,
        VrrPolicy::Always => true,
        VrrPolicy::Automatic => matches!(
            fullscreen,
            Some(ContentType::None | ContentType::Game | ContentType::Video)
        ),
    };
    let pacing = match (flip, fullscreen) {
        (Flip::Async, _) => Pacing::Immediate,
        (Flip::Vsync, Some(ContentType::Game)) => Pacing::LowLatency,
        (Flip::Vsync, Some(ContentType::Video)) => Pacing::Steady,
        (Flip::Vsync, Some(ContentType::None | ContentType::Photo) | None) => Pacing::Balanced,
    };
    Presentation { vrr, pacing }
}

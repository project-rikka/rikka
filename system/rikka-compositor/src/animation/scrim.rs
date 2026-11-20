use rikka_compositor_shell::LayerId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fader {
    Compositor,
    Client,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scrim {
    pub(crate) layer: LayerId,
    pub(crate) frosted: bool,
    pub(crate) fader: Fader,
}

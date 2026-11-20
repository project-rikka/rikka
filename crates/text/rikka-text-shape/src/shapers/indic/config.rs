use rikka_text_unicode::Script;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RephPosition {
    AfterMain,
    BeforeSub,
    AfterSub,
    BeforePost,
    AfterPost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RephMode {
    Implicit,
    Explicit,
    Logical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BelowFormsMode {
    PreAndPost,
    PostOnly,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct IndicConfig {
    pub(crate) has_old_spec: bool,
    pub(crate) virama: u32,
    pub(crate) reph_position: RephPosition,
    pub(crate) reph_mode: RephMode,
    pub(crate) below_forms: BelowFormsMode,
}

const fn config(
    virama: u32,
    reph_position: RephPosition,
    reph_mode: RephMode,
    below_forms: BelowFormsMode,
) -> IndicConfig {
    IndicConfig {
        has_old_spec: true,
        virama,
        reph_position,
        reph_mode,
        below_forms,
    }
}

impl IndicConfig {
    pub(crate) fn for_script(script: Script) -> Self {
        use BelowFormsMode::{PostOnly, PreAndPost};
        use RephMode::{Explicit, Implicit, Logical};
        use RephPosition::{AfterMain, AfterPost, AfterSub, BeforePost, BeforeSub};
        match script {
            Script::Devanagari => config(0x094D, BeforePost, Implicit, PreAndPost),
            Script::Bengali => config(0x09CD, AfterSub, Implicit, PreAndPost),
            Script::Gurmukhi => config(0x0A4D, BeforeSub, Implicit, PreAndPost),
            Script::Gujarati => config(0x0ACD, BeforePost, Implicit, PreAndPost),
            Script::Oriya => config(0x0B4D, AfterMain, Implicit, PreAndPost),
            Script::Tamil => config(0x0BCD, AfterPost, Implicit, PreAndPost),
            Script::Telugu => config(0x0C4D, AfterPost, Explicit, PostOnly),
            Script::Kannada => config(0x0CCD, AfterPost, Implicit, PostOnly),
            Script::Malayalam => config(0x0D4D, AfterMain, Logical, PreAndPost),
            _ => IndicConfig {
                has_old_spec: false,
                virama: 0,
                reph_position: BeforePost,
                reph_mode: Implicit,
                below_forms: PreAndPost,
            },
        }
    }
}

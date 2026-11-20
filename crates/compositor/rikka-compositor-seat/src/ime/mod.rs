mod composing;
mod focus;
mod host;
mod input_method;
mod popup;
mod text_input;

use rikka_compositor_protocols::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use rikka_compositor_protocols::zwp_input_method_v2::ZwpInputMethodV2;
use rikka_compositor_surface::SurfaceId;

pub(crate) use composing::Composing;
pub use composing::{Surrounding, TextInputState};
pub(crate) use focus::{focus_entered, focus_left};
pub(crate) use host::{compose, serve_text_input};
pub(crate) use input_method::{
    grab_destroyed, grab_request, input_method_destroyed, input_method_manager,
    input_method_request,
};
pub(crate) use popup::{place_popup, popup_destroyed, popup_request, popups_surface_gone};
pub(crate) use text_input::{text_input_destroyed, text_input_manager, text_input_request};

use popup::Popup;
use text_input::TextInput;

use crate::keyboard::SentKeyboardState;

#[derive(Debug)]
pub(crate) struct MethodGrab {
    pub(crate) resource: ZwpInputMethodKeyboardGrabV2,
    pub(crate) sent: SentKeyboardState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composition {
    pub preedit: Option<(String, i32, i32)>,
    pub commit: Option<String>,
    pub delete: Option<(u32, u32)>,
}

#[derive(Debug)]
pub(crate) struct InputMethod {
    pub(crate) resource: ZwpInputMethodV2,
    pub(crate) pending: Composition,
}

#[derive(Debug, Default)]
pub(crate) struct Ime {
    pub(crate) text_inputs: Vec<TextInput>,
    pub(crate) method: Option<InputMethod>,
    pub(crate) grab: Option<MethodGrab>,
    pub(crate) popups: Vec<Popup>,
    pub(crate) composing: Composing,
    pub(crate) focus: Option<SurfaceId>,
    pub(crate) entered: bool,
    pub(crate) host: bool,
}

use rikka_color_formats::Rgb;
use rikka_seam::{Call, Manifest, Method, MethodInfo, Request, SeamError, Service, ServiceContext};
use rikka_settings::session::color_picker::{Activation, Button, Click};
use serde::{Deserialize, Serialize};
use shell_capture::Frozen;
use shell_kit::{Host, OPEN, Outputs, PresenceService, copy_to_clipboard};

use crate::CAPABILITY;
use crate::picker::Frame;
use crate::shared::Shared;

pub(super) const ACTIVATE: &str = "activate";

pub(crate) const PICK: &str = "pick";

pub(crate) const CHOSE: &str = "chose";

pub(crate) const CANCEL: &str = "cancel";

const SUMMARY: &str = "Colour picker and colour editor";

const SUBJECT: &str = "the colour editor";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) struct Chosen {
    pub(crate) color: Rgb,
    pub(crate) button: Button,
}

#[derive(Debug)]
pub(super) struct ColorPickerService {
    shared: Shared,
    editor: PresenceService,
}

impl ColorPickerService {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> ColorPickerService {
        let editor =
            PresenceService::new(CAPABILITY, SUMMARY, SUBJECT).with_presence(shared.editor.clone());
        ColorPickerService { shared, editor }
    }

    fn activate(&mut self, outputs: &Outputs, call: Call) {
        match self.shared.preferences.activation() {
            Activation::Pick => self.pick(outputs, call),
            Activation::Editor => self.editor.answer(Method::from(OPEN), call),
        }
    }

    fn pick(&mut self, outputs: &Outputs, call: Call) {
        if self.shared.picking.is_shown() {
            return call.done();
        }
        if self.editor.presence().is_shown() {
            self.editor.hide();
            let seam = self.shared.seam.clone();
            self.editor
                .presence()
                .after_unmap(move || seam.notify(Request::new(CAPABILITY, PICK)));
            return call.done();
        }
        match Frozen::take(&outputs.all(), false) {
            Ok(frozen) => {
                *self.shared.frames.borrow_mut() = frozen
                    .into_panels()
                    .into_iter()
                    .map(Frame::from_panel)
                    .collect();
                self.shared.picking.set(true);
                call.done();
            }
            Err(error) => {
                tracing::warn!("failed to freeze screen for colour picking: {error}");
                call.fail(SeamError::Unavailable(error.to_string()));
            }
        }
    }

    fn chose(&mut self, call: Call) {
        let chosen = match call.decode::<Chosen>() {
            Ok(chosen) => chosen,
            Err(error) => return call.fail(error),
        };
        self.put_away();
        match self.shared.preferences.click(chosen.button) {
            Click::Close => call.done(),
            Click::PickAndClose => {
                self.keep(chosen.color);
                call.done();
            }
            Click::PickThenEditor => {
                self.keep(chosen.color);
                self.editor.answer(Method::from(OPEN), call);
            }
        }
    }

    fn keep(&self, color: Rgb) {
        copy_to_clipboard(&self.shared.preferences.copied().render(color));
        self.shared.history.record(color);
    }

    fn put_away(&mut self) {
        self.shared.frames.borrow_mut().clear();
        self.shared.picking.set(false);
    }
}

impl Service<Host> for ColorPickerService {
    fn manifest(&self) -> Manifest {
        self.editor
            .manifest()
            .method(
                MethodInfo::new(ACTIVATE)
                    .summary("Runs the configured shortcut action: pick or open the editor"),
            )
            .method(MethodInfo::new(PICK).summary("Freezes the screen and opens the colour loupe"))
            .method(MethodInfo::new(CHOSE).summary(
                "Picks `color` (`rrggbb`) with the mouse `button` (`primary`, `middle` or \
                 `secondary`), doing what that button is set to do",
            ))
            .method(MethodInfo::new(CANCEL).summary("Cancels the colour picker"))
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.editor.attach(seam);
    }

    fn call(&mut self, host: &mut Host, method: Method, call: Call) {
        match &*method {
            ACTIVATE => self.activate(host.outputs(), call),
            PICK => self.pick(host.outputs(), call),
            CHOSE => self.chose(call),
            CANCEL => {
                self.put_away();
                call.done();
            }
            settling => self.editor.answer(Method::from(settling), call),
        }
    }
}

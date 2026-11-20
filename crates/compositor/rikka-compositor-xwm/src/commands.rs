use std::borrow::Cow;
use std::os::fd::OwnedFd;

use x11rb_protocol::protocol::xproto::{
    CONFIGURE_NOTIFY_EVENT, ConfigureNotifyEvent, ConfigureWindowAux, ConfigureWindowRequest,
    EventMask, InputFocus, KillClientRequest, MapWindowRequest, SetInputFocusRequest, StackMode,
};

use crate::error::Result;
use crate::event::{ConfigureSerial, XwmEvent};
use crate::fault::Outcome;
use crate::frame_extents::FrameExtents;
use crate::geometry::Geometry;
use crate::selection::{MimeList, SelectionKind};
use crate::stacking::{Placement, Restack};
use crate::window::{Protocols, WindowStates, XWindowId};
use crate::wire::{CURRENT_TIME, NONE};
use crate::wm_state::IcccmState;
use crate::xsettings::XSetting;
use crate::xwm::Xwm;

impl Xwm {
    pub fn map(&mut self, window: XWindowId) -> Result<()> {
        let hidden = self.managed(window)?.is_minimized();
        let state = if hidden {
            IcccmState::Iconic
        } else {
            IcccmState::Normal
        };
        self.write_icccm_state(window, state)?;
        self.link.wire.send(MapWindowRequest {
            window: window.raw(),
        })?;
        Ok(())
    }

    pub fn set_frame_extents(&mut self, window: XWindowId, extents: FrameExtents) -> Result<()> {
        self.tracked(window)?;
        let atom = self.link.atoms._NET_FRAME_EXTENTS;
        self.link
            .set_cardinals(window.raw(), atom, &extents.cardinals())?;
        Ok(())
    }

    pub fn configure(&mut self, window: XWindowId, geometry: Geometry) -> Result<ConfigureSerial> {
        let settled = self.managed(window)?.geometry;
        let geometry = geometry.clamped();
        let expected = self.configures.target(window, settled);
        let serial = self.configures.next_serial();
        let sequence = self.link.wire.send(ConfigureWindowRequest {
            window: window.raw(),
            value_list: Cow::Owned(
                ConfigureWindowAux::new()
                    .x(geometry.x)
                    .y(geometry.y)
                    .width(geometry.width)
                    .height(geometry.height)
                    .border_width(0),
            ),
        })?;
        if expected == geometry {
            self.events.push(XwmEvent::ConfigureAccepted {
                window,
                serial,
                geometry,
            });
        } else {
            self.configures.expect(window, sequence, geometry, serial);
        }
        self.send_synthetic_configure(window, geometry)?;
        Ok(serial)
    }

    pub fn focus(&mut self, window: Option<XWindowId>) -> Result<()> {
        if let Some(window) = window {
            self.managed(window)?;
        }
        if let Some(previous) = self.focused
            && Some(previous) != window
        {
            self.change_states(previous, WindowStates::FOCUSED, false)?;
        }
        self.focused = window;
        self.refocus()?;
        if let Some(window) = window {
            self.change_states(window, WindowStates::FOCUSED, true)?;
        }
        self.publish_active(window)?;
        Ok(())
    }

    pub fn close(&mut self, window: XWindowId) -> Result<()> {
        if self
            .tracked(window)?
            .protocols
            .contains(Protocols::DELETE_WINDOW)
        {
            self.protocol_message(window, self.link.atoms.WM_DELETE_WINDOW)?;
        } else {
            self.link.wire.send(KillClientRequest {
                resource: window.raw(),
            })?;
        }
        Ok(())
    }

    pub fn raise(&mut self, window: XWindowId) -> Result<()> {
        self.managed(window)?;
        let moves = self.stacking.raise(window);
        self.restacked(&moves)
    }

    pub fn restack(&mut self, bottom_to_top: impl IntoIterator<Item = XWindowId>) -> Result<()> {
        let order: Vec<XWindowId> = bottom_to_top.into_iter().collect();
        let moves = self.stacking.restack(&order);
        self.restacked(&moves)
    }

    pub fn set_maximized(&mut self, window: XWindowId, maximized: bool) -> Result<()> {
        self.managed(window)?;
        self.change_states(window, WindowStates::MAXIMIZED, maximized)?;
        Ok(())
    }

    pub fn set_fullscreen(&mut self, window: XWindowId, fullscreen: bool) -> Result<()> {
        self.managed(window)?;
        self.change_states(window, WindowStates::FULLSCREEN, fullscreen)?;
        Ok(())
    }

    pub fn set_minimized(&mut self, window: XWindowId, minimized: bool) -> Result<()> {
        let mapped = self.managed(window)?.mapped;
        self.change_states(window, WindowStates::HIDDEN, minimized)?;
        if mapped {
            let state = if minimized {
                IcccmState::Iconic
            } else {
                IcccmState::Normal
            };
            self.write_icccm_state(window, state)?;
        }
        Ok(())
    }

    pub fn set_selection(&mut self, kind: SelectionKind, mime_types: MimeList) -> Result<()> {
        let (selections, mut io) = self.split();
        selections.offer(&mut io, kind, mime_types)?;
        Ok(())
    }

    pub fn clear_selection(&mut self, kind: SelectionKind) -> Result<()> {
        self.selections.withdraw(&mut self.link, kind)?;
        Ok(())
    }

    pub fn request_selection(
        &mut self,
        kind: SelectionKind,
        mime_type: &str,
        fd: OwnedFd,
    ) -> Result<()> {
        let (selections, mut io) = self.split();
        selections.fetch(&mut io, kind, mime_type, fd)
    }

    pub fn set_xsetting(&mut self, name: &str, value: XSetting) -> Result<()> {
        if self.xsettings.set(name, value)? {
            self.publish_settings()?;
        }
        Ok(())
    }

    pub(crate) fn refocus(&mut self) -> Outcome {
        let Some(window) = self.focused.and_then(|window| self.window(window)) else {
            return self.set_input_focus(NONE);
        };
        let (id, model) = (window.id, window.input_model());
        if model.sets_input_focus() {
            self.set_input_focus(id.raw())?;
        } else if !model.takes_focus() {
            self.set_input_focus(NONE)?;
        }
        if model.takes_focus() {
            self.ask_to_take_focus(id)?;
        }
        Ok(())
    }

    fn set_input_focus(&mut self, focus: u32) -> Outcome {
        self.link.wire.send(SetInputFocusRequest {
            revert_to: InputFocus::POINTER_ROOT,
            focus,
            time: CURRENT_TIME,
        })?;
        Ok(())
    }

    fn restacked(&mut self, moves: &[Restack]) -> Result<()> {
        if moves.is_empty() {
            return Ok(());
        }
        for restack in moves {
            let (sibling, mode) = match restack.placement {
                Placement::Above(sibling) => (sibling, StackMode::ABOVE),
                Placement::Below(sibling) => (sibling, StackMode::BELOW),
            };
            self.link.wire.send(ConfigureWindowRequest {
                window: restack.window.raw(),
                value_list: Cow::Owned(
                    ConfigureWindowAux::new()
                        .sibling(sibling.raw())
                        .stack_mode(mode),
                ),
            })?;
        }
        self.publish_clients()?;
        Ok(())
    }

    fn send_synthetic_configure(&mut self, window: XWindowId, geometry: Geometry) -> Outcome {
        self.link.send_event(
            window.raw(),
            EventMask::STRUCTURE_NOTIFY,
            ConfigureNotifyEvent {
                response_type: CONFIGURE_NOTIFY_EVENT,
                sequence: 0,
                event: window.raw(),
                window: window.raw(),
                above_sibling: NONE,
                x: i16::try_from(geometry.x).unwrap_or_default(),
                y: i16::try_from(geometry.y).unwrap_or_default(),
                width: u16::try_from(geometry.width).unwrap_or(u16::MAX),
                height: u16::try_from(geometry.height).unwrap_or(u16::MAX),
                border_width: 0,
                override_redirect: false,
            },
        )
    }
}

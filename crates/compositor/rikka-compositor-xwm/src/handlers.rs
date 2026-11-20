use std::borrow::Cow;

use x11rb_protocol::protocol::Event;
use x11rb_protocol::protocol::xproto::{
    ChangeWindowAttributesAux, ChangeWindowAttributesRequest, ConfigWindow, ConfigureNotifyEvent,
    ConfigureRequestEvent, ConfigureWindowAux, ConfigureWindowRequest, CreateNotifyEvent,
    EventMask, FocusInEvent, GetGeometryReply, GetGeometryRequest, MapNotifyEvent,
    MapWindowRequest, NotifyDetail, NotifyMode, PropertyNotifyEvent, ReparentNotifyEvent,
    StackMode,
};

use crate::event::{StackRequest, XwmEvent};
use crate::fault::{Outcome, describe};
use crate::geometry::Geometry;
use crate::pending::ExpectedReply;
use crate::record::{Record, current};
use crate::selection::Access;
use crate::window::{XWindow, XWindowId};
use crate::wire::{Reply, parse};
use crate::wm_state::IcccmState;
use crate::xwm::{Xwm, client_events};

const SENT_EVENT: u8 = 0x80;

impl Xwm {
    pub(crate) fn handle(&mut self, event: Event, sequence: u64) -> Outcome {
        match event {
            Event::CreateNotify(created) => self.created(&created),
            Event::ReparentNotify(reparented) => self.reparented(&reparented),
            Event::MapRequest(request) => self.map_requested(XWindowId::new(request.window)),
            Event::MapNotify(mapped) => self.map_notified(&mapped),
            Event::UnmapNotify(unmapped) => self.unmapped(XWindowId::new(unmapped.window)),
            Event::DestroyNotify(destroyed) => {
                self.selections
                    .requestor_gone(&self.poller, destroyed.window);
                self.forget(XWindowId::new(destroyed.window))
            }
            Event::ConfigureRequest(request) => self.configure_requested(&request),
            Event::ConfigureNotify(notify) => self.configure_notified(&notify, sequence),
            Event::PropertyNotify(change) => self.property_notified(&change),
            Event::ClientMessage(message) => match self.dnd_message(&message)? {
                true => Ok(()),
                false => self.client_message(&message),
            },
            Event::FocusIn(focus) => self.focus_in(&focus),
            Event::XfixesSelectionNotify(change) => {
                let (selections, mut io) = self.split();
                selections.owner_changed(&mut io, &change)
            }
            Event::SelectionNotify(notify) => {
                let (selections, mut io) = self.split();
                selections.converted(&mut io, &notify)
            }
            Event::SelectionRequest(request) => {
                let access = Access {
                    allowed: self.focused.is_some(),
                    requestor_events: if self
                        .windows
                        .contains_key(&XWindowId::new(request.requestor))
                    {
                        client_events()
                    } else {
                        EventMask::PROPERTY_CHANGE
                    },
                };
                let (selections, mut io) = self.split();
                selections.requested(&mut io, &request, access)
            }
            Event::SelectionClear(clear) => {
                self.selections.lost(clear.owner, clear.selection);
                Ok(())
            }
            Event::Error(error) => {
                tracing::debug!("X11 error: {}", describe(&error));
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn geometry_read(&mut self, window: XWindowId, epoch: u64, reply: Reply) -> Outcome {
        let Some(record) = current(&mut self.windows, window, epoch) else {
            return Ok(());
        };
        record.reading -= 1;
        match reply {
            Ok(packet) => {
                let shape: GetGeometryReply = parse(&packet)?;
                record.window.geometry = Geometry::new(
                    i32::from(shape.x),
                    i32::from(shape.y),
                    u32::from(shape.width),
                    u32::from(shape.height),
                );
            }
            Err(error) => {
                tracing::debug!("failed to get geometry of {window}: {}", describe(&error));
            }
        }
        Ok(())
    }

    fn created(&mut self, created: &CreateNotifyEvent) -> Outcome {
        let geometry = Geometry::new(
            i32::from(created.x),
            i32::from(created.y),
            u32::from(created.width),
            u32::from(created.height),
        );
        self.adopt(
            created.window,
            created.parent,
            geometry,
            created.override_redirect,
        )?;
        Ok(())
    }

    fn reparented(&mut self, reparented: &ReparentNotifyEvent) -> Outcome {
        let window = XWindowId::new(reparented.window);
        if reparented.parent != self.link.root {
            return self.forget(window);
        }
        let geometry = Geometry::new(i32::from(reparented.x), i32::from(reparented.y), 0, 0);
        let Some(epoch) = self.adopt(
            reparented.window,
            reparented.parent,
            geometry,
            reparented.override_redirect,
        )?
        else {
            return Ok(());
        };
        let sequence = self.link.wire.ask(GetGeometryRequest {
            drawable: reparented.window,
        })?;
        self.pending
            .expect(sequence, ExpectedReply::Geometry { window, epoch });
        if let Some(record) = self.windows.get_mut(&window) {
            record.reading += 1;
        }
        Ok(())
    }

    fn adopt(
        &mut self,
        raw: u32,
        parent: u32,
        geometry: Geometry,
        override_redirect: bool,
    ) -> Outcome<Option<u64>> {
        let window = XWindowId::new(raw);
        if parent != self.link.root || self.link.is_ours(raw) || self.windows.contains_key(&window)
        {
            return Ok(None);
        }
        self.link.wire.send(ChangeWindowAttributesRequest {
            window: raw,
            value_list: Cow::Owned(ChangeWindowAttributesAux::new().event_mask(client_events())),
        })?;
        self.epochs += 1;
        let epoch = self.epochs;
        self.windows.insert(
            window,
            Record::new(XWindow::new(window, geometry, override_redirect), epoch),
        );
        self.read_all(window)?;
        Ok(Some(epoch))
    }

    fn map_requested(&mut self, window: XWindowId) -> Outcome {
        match self.window(window) {
            Some(tracked) if !tracked.override_redirect => {
                self.events.push(XwmEvent::MapRequest { window });
            }
            _ => {
                self.link.wire.send(MapWindowRequest {
                    window: window.raw(),
                })?;
            }
        }
        Ok(())
    }

    fn map_notified(&mut self, mapped: &MapNotifyEvent) -> Outcome {
        let window = XWindowId::new(mapped.window);
        let Some(record) = self.windows.get_mut(&window) else {
            return Ok(());
        };
        let tracked = &mut record.window;
        if tracked.mapped {
            return Ok(());
        }
        tracked.mapped = true;
        tracked.override_redirect = mapped.override_redirect;
        if !mapped.override_redirect && self.stacking.mapped(window) {
            self.publish_clients()?;
        }
        self.events.push(XwmEvent::Mapped { window });
        Ok(())
    }

    fn unmapped(&mut self, window: XWindowId) -> Outcome {
        let Some(record) = self.windows.get_mut(&window) else {
            return Ok(());
        };
        let tracked = &mut record.window;
        let was_mapped = std::mem::replace(&mut tracked.mapped, false);
        tracked.surface_serial = None;
        let managed = !tracked.override_redirect;
        if self.stacking.unmapped(window) {
            self.publish_clients()?;
        }
        if managed {
            self.write_icccm_state(window, IcccmState::Withdrawn)?;
        }
        self.withdraw_focus(window)?;
        if was_mapped {
            self.events.push(XwmEvent::Unmapped { window });
        }
        Ok(())
    }

    pub(crate) fn forget(&mut self, window: XWindowId) -> Outcome {
        let Some(record) = self.windows.remove(&window) else {
            return Ok(());
        };
        if self.stacking.unmapped(window) {
            self.publish_clients()?;
        }
        self.configures.forget(window);
        if self.focused == Some(window) {
            self.focused = None;
            self.publish_active(None)?;
        }
        if record.window.mapped {
            self.events.push(XwmEvent::Unmapped { window });
        }
        self.events.push(XwmEvent::Destroyed { window });
        Ok(())
    }

    fn configure_requested(&mut self, request: &ConfigureRequestEvent) -> Outcome {
        let window = XWindowId::new(request.window);
        let asks = |field: ConfigWindow| request.value_mask.contains(field);
        let deciding = self
            .window(window)
            .is_some_and(|tracked| tracked.mapped && !tracked.override_redirect);
        if !deciding {
            let granted = ConfigureWindowAux::from_configure_request(request);
            self.link.wire.send(ConfigureWindowRequest {
                window: request.window,
                value_list: Cow::Owned(granted),
            })?;
            return Ok(());
        }
        let sibling = asks(ConfigWindow::SIBLING).then(|| XWindowId::new(request.sibling));
        let stack = match (asks(ConfigWindow::STACK_MODE), request.stack_mode, sibling) {
            (false, _, _) => None,
            (true, StackMode::ABOVE, None) => Some(StackRequest::Top),
            (true, StackMode::ABOVE, Some(sibling)) => Some(StackRequest::Above(sibling)),
            (true, StackMode::BELOW, None) => Some(StackRequest::Bottom),
            (true, StackMode::BELOW, Some(sibling)) => Some(StackRequest::Below(sibling)),
            (true, _, _) => None,
        };
        self.events.push(XwmEvent::ConfigureRequest {
            window,
            x: asks(ConfigWindow::X).then_some(i32::from(request.x)),
            y: asks(ConfigWindow::Y).then_some(i32::from(request.y)),
            width: asks(ConfigWindow::WIDTH).then_some(u32::from(request.width)),
            height: asks(ConfigWindow::HEIGHT).then_some(u32::from(request.height)),
            stack,
        });
        Ok(())
    }

    fn configure_notified(&mut self, notify: &ConfigureNotifyEvent, sequence: u64) -> Outcome {
        if notify.response_type & SENT_EVENT != 0 {
            return Ok(());
        }
        let window = XWindowId::new(notify.window);
        let Some(record) = self.windows.get_mut(&window) else {
            return Ok(());
        };
        let geometry = Geometry::new(
            i32::from(notify.x),
            i32::from(notify.y),
            u32::from(notify.width),
            u32::from(notify.height),
        );
        let moved = record.window.geometry != geometry;
        record.window.geometry = geometry;
        if let Some(serial) = self.configures.settle(window, sequence, geometry) {
            self.events.push(XwmEvent::ConfigureAccepted {
                window,
                serial,
                geometry,
            });
        } else if moved {
            self.events.push(XwmEvent::Configured { window, geometry });
        }
        Ok(())
    }

    fn property_notified(&mut self, change: &PropertyNotifyEvent) -> Outcome {
        let (selections, mut io) = self.split();
        selections.property_changed(&mut io, change)?;
        self.property_changed(XWindowId::new(change.window), change.atom);
        Ok(())
    }

    fn focus_in(&mut self, focus: &FocusInEvent) -> Outcome {
        if matches!(focus.mode, NotifyMode::GRAB | NotifyMode::UNGRAB)
            || focus.detail == NotifyDetail::POINTER
        {
            return Ok(());
        }
        let window = XWindowId::new(focus.event);
        if self.focused == Some(window) {
            return Ok(());
        }
        let same_client = self.focused.is_some_and(|focused| {
            self.window(focused)
                .is_some_and(|tracked| tracked.same_client(window, self.link.client_mask()))
        });
        if same_client && self.windows.contains_key(&window) {
            self.events.push(XwmEvent::ActivateRequest { window });
            return Ok(());
        }
        self.refocus()
    }
}

use rikka_types::ResizeEdge;
use x11rb_protocol::protocol::xproto::ClientMessageEvent;

use crate::event::XwmEvent;
use crate::ewmh::state_of;
use crate::fault::Outcome;
use crate::window::{WindowStates, XWindowId};
use crate::xwm::Xwm;

const REMOVE: u32 = 0;
const ADD: u32 = 1;
const TOGGLE: u32 = 2;

const ICONIC: u32 = 3;
const NORMAL: u32 = 1;

const MOVE: u32 = 8;

const CLIENT_OWNED: WindowStates = WindowStates::MODAL
    .union(WindowStates::SKIP_TASKBAR)
    .union(WindowStates::SKIP_PAGER)
    .union(WindowStates::DEMANDS_ATTENTION);

impl Xwm {
    pub(crate) fn client_message(&mut self, message: &ClientMessageEvent) -> Outcome {
        let window = XWindowId::new(message.window);
        if !self.windows.contains_key(&window) {
            return Ok(());
        }
        let data = message.data.as_data32();
        let atoms = self.link.atoms;
        match message.type_ {
            kind if kind == atoms.WL_SURFACE_SERIAL => {
                let serial = u64::from(data[0]) | (u64::from(data[1]) << 32);
                if let Some(record) = self.windows.get_mut(&window) {
                    record.window.surface_serial = Some(serial);
                }
                self.events.push(XwmEvent::SurfaceSerial { window, serial });
            }
            kind if kind == atoms._NET_WM_STATE => self.state_message(window, data)?,
            kind if kind == atoms.WM_CHANGE_STATE => match data[0] {
                ICONIC => self.events.push(XwmEvent::MinimizeRequest {
                    window,
                    minimized: true,
                }),
                NORMAL => self.events.push(XwmEvent::MinimizeRequest {
                    window,
                    minimized: false,
                }),
                _ => {}
            },
            kind if kind == atoms._NET_WM_MOVERESIZE => {
                let button = data[3];
                if data[2] == MOVE {
                    self.events.push(XwmEvent::MoveRequest { window, button });
                } else if let Some(edge) = resize_edge(data[2]) {
                    self.events.push(XwmEvent::ResizeRequest {
                        window,
                        button,
                        edge,
                    });
                }
            }
            kind if kind == atoms._NET_ACTIVE_WINDOW => {
                self.events.push(XwmEvent::ActivateRequest { window });
            }
            kind if kind == atoms._NET_REQUEST_FRAME_EXTENTS => {
                self.events.push(XwmEvent::FrameExtentsRequest { window });
            }
            _ => {}
        }
        Ok(())
    }

    fn state_message(&mut self, window: XWindowId, data: [u32; 5]) -> Outcome {
        let Some(tracked) = self.window(window) else {
            return Ok(());
        };
        let atoms = &self.link.atoms;
        let named = state_of(atoms, data[1]) | state_of(atoms, data[2]);
        let current = tracked.states;
        let mut target_states = current;
        for state in named.iter() {
            let on = match data[0] {
                REMOVE => false,
                ADD => true,
                TOGGLE => !current.contains(state),
                _ => return Ok(()),
            };
            target_states.set(state, on);
        }
        if !tracked.mapped {
            self.change_states(window, target_states - current, true)?;
            self.change_states(window, current - target_states, false)?;
            return Ok(());
        }
        let client_owned = named & CLIENT_OWNED;
        if !client_owned.is_empty() {
            let on = target_states & client_owned;
            let off = client_owned - on;
            let raised = self.change_states(window, on, true)?;
            let lowered = self.change_states(window, off, false)?;
            let changed = raised || lowered;
            if changed {
                self.events.push(XwmEvent::PropertyChanged {
                    window,
                    property: crate::event::WindowProperty::States,
                });
            }
        }
        let flipped = |states: WindowStates| {
            named.intersects(states) && current.contains(states) != target_states.contains(states)
        };
        if named.intersects(WindowStates::MAXIMIZED)
            && current.contains(WindowStates::MAXIMIZED)
                != target_states.contains(WindowStates::MAXIMIZED)
        {
            self.events.push(XwmEvent::MaximizeRequest {
                window,
                maximized: target_states.contains(WindowStates::MAXIMIZED),
            });
        }
        if flipped(WindowStates::FULLSCREEN) {
            self.events.push(XwmEvent::FullscreenRequest {
                window,
                fullscreen: target_states.contains(WindowStates::FULLSCREEN),
            });
        }
        if flipped(WindowStates::HIDDEN) {
            self.events.push(XwmEvent::MinimizeRequest {
                window,
                minimized: target_states.contains(WindowStates::HIDDEN),
            });
        }
        if flipped(WindowStates::ABOVE) {
            self.events.push(XwmEvent::AboveRequest {
                window,
                above: target_states.contains(WindowStates::ABOVE),
            });
        }
        Ok(())
    }
}

fn resize_edge(direction: u32) -> Option<ResizeEdge> {
    Some(match direction {
        0 => ResizeEdge::TopLeft,
        1 => ResizeEdge::Top,
        2 => ResizeEdge::TopRight,
        3 => ResizeEdge::Right,
        4 => ResizeEdge::BottomRight,
        5 => ResizeEdge::Bottom,
        6 => ResizeEdge::BottomLeft,
        7 => ResizeEdge::Left,
        _ => return None,
    })
}

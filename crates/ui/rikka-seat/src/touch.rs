use rikka_input::DeviceEvent;
use rikka_types::{Modifiers, Point, PointerButton};
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wl_touch::{self, WlTouch};

use crate::seat_event::SeatEvent;

const RELEASE_SINCE_VERSION: u32 = 3;

#[derive(Debug)]
struct Primary {
    id: i32,
    surface: WlSurface,
    position: Point,
}

#[derive(Debug)]
pub(crate) struct Touch {
    pub(crate) touch: WlTouch,
    primary: Option<Primary>,
}

impl Touch {
    pub(crate) fn new(touch: WlTouch) -> Touch {
        Touch {
            touch,
            primary: None,
        }
    }

    pub(crate) fn release(self) {
        if self.touch.version() >= RELEASE_SINCE_VERSION {
            self.touch.release();
        }
    }

    pub(crate) fn handle(
        &mut self,
        event: wl_touch::Event,
        modifiers: Modifiers,
        out: &mut Vec<SeatEvent>,
    ) -> Option<u32> {
        match event {
            wl_touch::Event::Down {
                serial,
                surface,
                id,
                x,
                y,
                ..
            } => {
                if self.primary.is_some() {
                    return None;
                }
                let position = Point::new(x as f32, y as f32);
                out.push(SeatEvent::Input {
                    surface: surface.clone(),
                    event: DeviceEvent::PointerMoved(position),
                });
                out.push(SeatEvent::Input {
                    surface: surface.clone(),
                    event: DeviceEvent::PointerPressed {
                        position,
                        button: PointerButton::Primary,
                        modifiers,
                    },
                });
                self.primary = Some(Primary {
                    id,
                    surface,
                    position,
                });
                return Some(serial);
            }
            wl_touch::Event::Motion { id, x, y, .. } => {
                let primary = self.primary.as_mut().filter(|primary| primary.id == id)?;
                primary.position = Point::new(x as f32, y as f32);
                out.push(SeatEvent::Input {
                    surface: primary.surface.clone(),
                    event: DeviceEvent::PointerMoved(primary.position),
                });
            }
            wl_touch::Event::Up { id, .. } => {
                let primary = self.primary.take_if(|primary| primary.id == id)?;
                out.push(SeatEvent::Input {
                    surface: primary.surface.clone(),
                    event: DeviceEvent::PointerReleased {
                        position: primary.position,
                        button: PointerButton::Primary,
                        modifiers,
                    },
                });
                out.push(SeatEvent::Input {
                    surface: primary.surface,
                    event: DeviceEvent::PointerLeft,
                });
            }
            wl_touch::Event::Cancel => {
                if let Some(primary) = self.primary.take() {
                    out.push(SeatEvent::Input {
                        surface: primary.surface,
                        event: DeviceEvent::PointerLeft,
                    });
                }
            }
            _ => {}
        }
        None
    }
}

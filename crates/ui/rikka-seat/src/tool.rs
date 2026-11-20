use std::time::Duration;

use rikka_input::{DeviceEvent, ScrollSource, evdev};
use rikka_types::{
    CursorShape, Modifiers, Point, PointerButton, SCROLL_LINE, ScrollDelta, Stylus, StylusSample,
    Tilt, Tool, ToolKind,
};
use rikka_wayland::SerialKind;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_tablet_tool_v2::{
    self, ButtonState, Capability, Type, ZwpTabletToolV2,
};

use crate::cursor::{Cursor, Cursors, Owner};
use crate::seat_event::SeatEvent;

const AXIS_RANGE: f32 = 65535.0;

const WHEEL_DEGREES_PER_CLICK: f32 = 15.0;

#[derive(Debug, Default)]
struct Axes {
    position: Point,
    pressure: Option<f32>,
    distance: Option<f32>,
    tilt: Option<Tilt>,
    rotation: Option<f32>,
    slider: Option<f32>,
}

#[derive(Debug, Default)]
struct Frame {
    entered: Option<WlSurface>,
    left: bool,
    down: bool,
    up: bool,
    changed: bool,
    buttons: Vec<(PointerButton, bool)>,
    wheel: Option<(f32, i32)>,
}

impl Frame {
    fn is_discrete(&self) -> bool {
        self.entered.is_some()
            || self.left
            || self.down
            || self.up
            || !self.buttons.is_empty()
            || self.wheel.is_some()
    }
}

#[derive(Debug)]
struct Coalesced {
    surface: WlSurface,
    stylus: Stylus,
}

#[derive(Debug)]
pub(crate) struct ToolDevice {
    pub(crate) proxy: ZwpTabletToolV2,
    cursor: Cursor,
    tool: Tool,
    focus: Option<WlSurface>,
    entered: u32,
    shown: Option<(CursorShape, u32)>,
    axes: Axes,
    contact: bool,
    held: Vec<PointerButton>,
    frame: Frame,
    coalesced: Option<Coalesced>,
}

impl ToolDevice {
    pub(crate) fn new(proxy: ZwpTabletToolV2, cursor: Cursor) -> ToolDevice {
        ToolDevice {
            proxy,
            cursor,
            tool: Tool::new(ToolKind::Pen),
            focus: None,
            entered: 0,
            shown: None,
            axes: Axes::default(),
            contact: false,
            held: Vec::new(),
            frame: Frame::default(),
            coalesced: None,
        }
    }

    pub(crate) fn release(self) {
        self.cursor.release();
        self.proxy.destroy();
    }

    pub(crate) fn completes_discrete_frame(&self, event: &zwp_tablet_tool_v2::Event) -> bool {
        matches!(event, zwp_tablet_tool_v2::Event::Frame { .. }) && self.frame.is_discrete()
    }

    pub(crate) fn show(
        &mut self,
        surface: &WlSurface,
        shape: CursorShape,
        scale: u32,
        cursors: &mut Cursors,
    ) {
        if self.focus.as_ref() != Some(surface) || self.shown == Some((shape, scale)) {
            return;
        }
        self.shown = Some((shape, scale));
        self.cursor.show(
            Owner::Tool(&self.proxy),
            self.entered,
            shape,
            scale,
            cursors,
        );
    }

    pub(crate) fn flush(&mut self, out: &mut Vec<SeatEvent>) {
        if let Some(Coalesced { surface, stylus }) = self.coalesced.take() {
            out.push(SeatEvent::Stylus {
                surface,
                event: DeviceEvent::PointerMoved(stylus.sample().position),
                stylus,
            });
        }
    }

    pub(crate) fn removed(&mut self, modifiers: Modifiers, out: &mut Vec<SeatEvent>) {
        self.flush(out);
        if self.focus.is_some() {
            self.frame = Frame {
                left: true,
                ..Frame::default()
            };
            self.frame(Duration::ZERO, modifiers, out);
        }
    }

    pub(crate) fn handle(
        &mut self,
        event: zwp_tablet_tool_v2::Event,
        modifiers: Modifiers,
        out: &mut Vec<SeatEvent>,
    ) -> Option<(SerialKind, u32)> {
        match event {
            zwp_tablet_tool_v2::Event::Type { tool_type } => match tool_type {
                Ok(kind) => self.tool.kind = tool_kind(kind),
                Err(unknown) => tracing::warn!("unknown tablet tool type: {unknown:?}"),
            },
            zwp_tablet_tool_v2::Event::HardwareSerial {
                hardware_serial_hi,
                hardware_serial_lo,
            } => self.tool.serial = wide(hardware_serial_hi, hardware_serial_lo),
            zwp_tablet_tool_v2::Event::HardwareIdWacom {
                hardware_id_hi,
                hardware_id_lo,
            } => self.tool.hardware_id = wide(hardware_id_hi, hardware_id_lo),
            zwp_tablet_tool_v2::Event::Capability { capability } => {
                let capabilities = &mut self.tool.capabilities;
                match capability {
                    Ok(Capability::Tilt) => capabilities.tilt = true,
                    Ok(Capability::Pressure) => capabilities.pressure = true,
                    Ok(Capability::Distance) => capabilities.distance = true,
                    Ok(Capability::Rotation) => capabilities.rotation = true,
                    Ok(Capability::Slider) => capabilities.slider = true,
                    Ok(Capability::Wheel) => capabilities.wheel = true,
                    unknown => tracing::warn!("unknown tablet tool capability: {unknown:?}"),
                }
            }
            zwp_tablet_tool_v2::Event::ProximityIn {
                serial, surface, ..
            } => {
                self.entered = serial;
                self.shown = None;
                self.contact = false;
                self.frame.entered = Some(surface);
                self.frame.changed = true;
            }
            zwp_tablet_tool_v2::Event::ProximityOut => self.frame.left = true,
            zwp_tablet_tool_v2::Event::Down { serial } => {
                self.contact = true;
                self.frame.down = true;
                return Some((SerialKind::ButtonPressed, serial));
            }
            zwp_tablet_tool_v2::Event::Up => {
                self.contact = false;
                self.frame.up = true;
            }
            zwp_tablet_tool_v2::Event::Motion { x, y } => {
                self.axes.position = Point::new(x as f32, y as f32);
                self.frame.changed = true;
            }
            zwp_tablet_tool_v2::Event::Pressure { pressure } => {
                self.axes.pressure = Some(normalized(pressure));
                self.frame.changed = true;
            }
            zwp_tablet_tool_v2::Event::Distance { distance } => {
                self.axes.distance = Some(normalized(distance));
                self.frame.changed = true;
            }
            zwp_tablet_tool_v2::Event::Tilt { tilt_x, tilt_y } => {
                self.axes.tilt = Some(Tilt::new(tilt_x as f32, tilt_y as f32));
                self.frame.changed = true;
            }
            zwp_tablet_tool_v2::Event::Rotation { degrees } => {
                self.axes.rotation = Some(degrees as f32);
                self.frame.changed = true;
            }
            zwp_tablet_tool_v2::Event::Slider { position } => {
                self.axes.slider = Some(position as f32 / AXIS_RANGE);
                self.frame.changed = true;
            }
            zwp_tablet_tool_v2::Event::Wheel { degrees, clicks } => {
                let (turned, clicked) = self.frame.wheel.unwrap_or_default();
                self.frame.wheel = Some((turned + degrees as f32, clicked + clicks));
            }
            zwp_tablet_tool_v2::Event::Button {
                serial,
                button,
                state,
            } => {
                let pressed = match state {
                    Ok(ButtonState::Pressed) => true,
                    Ok(ButtonState::Released) => false,
                    unknown => {
                        tracing::warn!("unknown tablet tool button state: {unknown:?}");
                        return None;
                    }
                };
                self.frame
                    .buttons
                    .push((evdev::tool_button(button), pressed));
                let kind = match pressed {
                    true => SerialKind::ButtonPressed,
                    false => SerialKind::ButtonReleased,
                };
                return Some((kind, serial));
            }
            zwp_tablet_tool_v2::Event::Frame { time } => {
                self.frame(Duration::from_millis(u64::from(time)), modifiers, out);
            }
            _ => {}
        }
        None
    }

    fn sample(&self, time: Duration) -> StylusSample {
        let capabilities = self.tool.capabilities;
        let declared = |declared: bool| declared.then_some(0.0);
        StylusSample {
            position: self.axes.position,
            pressure: self.axes.pressure.or(declared(capabilities.pressure)),
            distance: self.axes.distance.or(declared(capabilities.distance)),
            tilt: self
                .axes
                .tilt
                .or(capabilities.tilt.then_some(Tilt::default())),
            rotation: self.axes.rotation.or(declared(capabilities.rotation)),
            slider: self.axes.slider.or(declared(capabilities.slider)),
            contact: self.contact,
            time,
        }
    }

    fn coalesce(&mut self, surface: &WlSurface, sample: StylusSample) {
        match &mut self.coalesced {
            Some(coalesced) => coalesced.stylus.push(sample),
            None => {
                self.coalesced = Some(Coalesced {
                    surface: surface.clone(),
                    stylus: Stylus::new(self.tool, sample),
                });
            }
        }
    }

    fn frame(&mut self, time: Duration, modifiers: Modifiers, out: &mut Vec<SeatEvent>) {
        let frame = std::mem::take(&mut self.frame);
        let discrete = frame.is_discrete();
        if let Some(surface) = frame.entered.as_ref() {
            self.flush(out);
            self.focus = Some(surface.clone());
        }
        let Some(surface) = self.focus.clone() else {
            tracing::debug!("tablet tool frame without focused surface");
            return;
        };
        let sample = self.sample(time);
        if frame.changed {
            self.coalesce(&surface, sample);
        }
        if !discrete {
            return;
        }
        self.flush(out);
        let position = sample.position;
        let mut events = Vec::new();
        let held = &mut self.held;
        if frame.down {
            events.extend(transition(
                held,
                PointerButton::Primary,
                true,
                position,
                modifiers,
            ));
        }
        for (button, pressed) in frame.buttons {
            events.extend(transition(held, button, pressed, position, modifiers));
        }
        if frame.up {
            events.extend(transition(
                held,
                PointerButton::Primary,
                false,
                position,
                modifiers,
            ));
        }
        if let Some((degrees, clicks)) = frame.wheel {
            let lines = match clicks {
                0 => degrees / WHEEL_DEGREES_PER_CLICK,
                clicks => clicks as f32,
            };
            events.push(DeviceEvent::Scroll {
                position,
                delta: ScrollDelta::from_offset(Point::new(0.0, lines * SCROLL_LINE)),
                modifiers,
                source: ScrollSource::Wheel,
            });
        }
        if frame.left {
            for button in std::mem::take(held) {
                events.push(DeviceEvent::PointerReleased {
                    position,
                    button,
                    modifiers,
                });
            }
            events.push(DeviceEvent::PointerLeft);
            self.contact = false;
            self.focus = None;
        }
        let stylus = Stylus::new(self.tool, sample);
        out.extend(events.into_iter().map(|event| SeatEvent::Stylus {
            surface: surface.clone(),
            event,
            stylus: stylus.clone(),
        }));
    }
}

fn transition(
    held: &mut Vec<PointerButton>,
    button: PointerButton,
    pressed: bool,
    position: Point,
    modifiers: Modifiers,
) -> Option<DeviceEvent> {
    if pressed == held.contains(&button) {
        return None;
    }
    if pressed {
        held.push(button);
        Some(DeviceEvent::PointerPressed {
            position,
            button,
            modifiers,
        })
    } else {
        held.retain(|kept| *kept != button);
        Some(DeviceEvent::PointerReleased {
            position,
            button,
            modifiers,
        })
    }
}

fn tool_kind(kind: Type) -> ToolKind {
    match kind {
        Type::Pen => ToolKind::Pen,
        Type::Eraser => ToolKind::Eraser,
        Type::Brush => ToolKind::Brush,
        Type::Pencil => ToolKind::Pencil,
        Type::Airbrush => ToolKind::Airbrush,
        Type::Finger => ToolKind::Finger,
        Type::Mouse => ToolKind::Mouse,
        Type::Lens => ToolKind::Lens,
    }
}

fn wide(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

fn normalized(value: u32) -> f32 {
    (value as f32 / AXIS_RANGE).clamp(0.0, 1.0)
}

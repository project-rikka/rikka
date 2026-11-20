use std::time::Duration;

use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_tablet_pad_dial_v2::{self, ZwpTabletPadDialV2};
use rikka_wayland_client::zwp_tablet_pad_group_v2::{self, ZwpTabletPadGroupV2};
use rikka_wayland_client::zwp_tablet_pad_ring_v2::{self, ZwpTabletPadRingV2};
use rikka_wayland_client::zwp_tablet_pad_strip_v2::{self, ZwpTabletPadStripV2};
use rikka_wayland_client::zwp_tablet_pad_v2::{self, ZwpTabletPadV2};

use crate::seat_event::SeatEvent;

const STRIP_RANGE: f32 = 65535.0;

#[derive(Clone, Debug, PartialEq)]
pub enum PadEvent {
    Entered,
    Left,
    Button {
        button: u32,
        pressed: bool,
        time: Duration,
    },
    Ring {
        group: usize,
        ring: usize,
        degrees: Option<f32>,
        finger: bool,
        time: Duration,
    },
    Strip {
        group: usize,
        strip: usize,
        position: Option<f32>,
        finger: bool,
        time: Duration,
    },
    Dial {
        group: usize,
        dial: usize,
        value120: i32,
        time: Duration,
    },
    Mode {
        group: usize,
        mode: u32,
        time: Duration,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pad {
    pub buttons: u32,
    pub paths: Vec<String>,
    pub groups: Vec<PadGroup>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PadGroup {
    pub buttons: Vec<u32>,
    pub rings: usize,
    pub strips: usize,
    pub dials: usize,
    pub modes: u32,
    pub mode: u32,
}

#[derive(Debug, Default)]
struct Motion {
    value: Option<f32>,
    finger: bool,
    stopped: bool,
}

impl Motion {
    fn take(&mut self) -> Option<(Option<f32>, bool)> {
        let motion = std::mem::take(self);
        match (motion.value, motion.stopped) {
            (None, false) => None,
            (value, stopped) => Some((value.filter(|_| !stopped), motion.finger)),
        }
    }
}

#[derive(Debug)]
struct Ring {
    proxy: ZwpTabletPadRingV2,
    motion: Motion,
}

#[derive(Debug)]
struct Strip {
    proxy: ZwpTabletPadStripV2,
    motion: Motion,
}

#[derive(Debug)]
struct Dial {
    proxy: ZwpTabletPadDialV2,
    value120: i32,
}

#[derive(Debug)]
struct Group {
    proxy: ZwpTabletPadGroupV2,
    rings: Vec<Ring>,
    strips: Vec<Strip>,
    dials: Vec<Dial>,
}

#[derive(Debug)]
pub(crate) struct PadDevice {
    pub(crate) proxy: ZwpTabletPadV2,
    focus: Option<WlSurface>,
    pub(crate) info: Pad,
    groups: Vec<Group>,
}

impl PadDevice {
    pub(crate) fn new(proxy: ZwpTabletPadV2) -> PadDevice {
        PadDevice {
            proxy,
            focus: None,
            info: Pad::default(),
            groups: Vec::new(),
        }
    }

    pub(crate) fn release(self) {
        for group in self.groups {
            for ring in group.rings {
                ring.proxy.destroy();
            }
            for strip in group.strips {
                strip.proxy.destroy();
            }
            for dial in group.dials {
                dial.proxy.destroy();
            }
            group.proxy.destroy();
        }
        self.proxy.destroy();
    }

    pub(crate) fn removed(&mut self, out: &mut Vec<SeatEvent>) {
        if self.focus.is_some() {
            self.emit(PadEvent::Left, out);
            self.focus = None;
        }
    }

    pub(crate) fn owns_group(&self, group: &ZwpTabletPadGroupV2) -> bool {
        self.groups.iter().any(|owned| owned.proxy == *group)
    }

    pub(crate) fn owns_ring(&self, ring: &ZwpTabletPadRingV2) -> bool {
        self.groups
            .iter()
            .any(|group| group.rings.iter().any(|owned| owned.proxy == *ring))
    }

    pub(crate) fn owns_strip(&self, strip: &ZwpTabletPadStripV2) -> bool {
        self.groups
            .iter()
            .any(|group| group.strips.iter().any(|owned| owned.proxy == *strip))
    }

    pub(crate) fn owns_dial(&self, dial: &ZwpTabletPadDialV2) -> bool {
        self.groups
            .iter()
            .any(|group| group.dials.iter().any(|owned| owned.proxy == *dial))
    }

    fn emit(&self, event: PadEvent, out: &mut Vec<SeatEvent>) {
        if let Some(surface) = self.focus.clone() {
            out.push(SeatEvent::Pad { surface, event });
        }
    }

    pub(crate) fn handle(&mut self, event: zwp_tablet_pad_v2::Event, out: &mut Vec<SeatEvent>) {
        match event {
            zwp_tablet_pad_v2::Event::Group { pad_group } => {
                self.groups.push(Group {
                    proxy: pad_group,
                    rings: Vec::new(),
                    strips: Vec::new(),
                    dials: Vec::new(),
                });
                self.info.groups.push(PadGroup::default());
            }
            zwp_tablet_pad_v2::Event::Path { path } => self.info.paths.push(path),
            zwp_tablet_pad_v2::Event::Buttons { buttons } => self.info.buttons = buttons,
            zwp_tablet_pad_v2::Event::Button {
                time,
                button,
                state,
            } => {
                let pressed = match state {
                    Ok(zwp_tablet_pad_v2::ButtonState::Pressed) => true,
                    Ok(zwp_tablet_pad_v2::ButtonState::Released) => false,
                    unknown => {
                        tracing::warn!("unknown tablet pad button state: {unknown:?}");
                        return;
                    }
                };
                self.emit(
                    PadEvent::Button {
                        button,
                        pressed,
                        time: millis(time),
                    },
                    out,
                );
            }
            zwp_tablet_pad_v2::Event::Enter { surface, .. } => {
                self.focus = Some(surface);
                self.emit(PadEvent::Entered, out);
            }
            zwp_tablet_pad_v2::Event::Leave { surface, .. }
                if self.focus.as_ref() == Some(&surface) =>
            {
                self.emit(PadEvent::Left, out);
                self.focus = None;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_group(
        &mut self,
        proxy: &ZwpTabletPadGroupV2,
        event: zwp_tablet_pad_group_v2::Event,
        out: &mut Vec<SeatEvent>,
    ) {
        let Some(index) = self.groups.iter().position(|group| group.proxy == *proxy) else {
            return;
        };
        let (group, info) = (&mut self.groups[index], &mut self.info.groups[index]);
        match event {
            zwp_tablet_pad_group_v2::Event::Buttons { buttons } => {
                info.buttons = buttons
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|bytes| u32::from_ne_bytes(*bytes))
                    .collect();
            }
            zwp_tablet_pad_group_v2::Event::Ring { ring } => {
                group.rings.push(Ring {
                    proxy: ring,
                    motion: Motion::default(),
                });
                info.rings = group.rings.len();
            }
            zwp_tablet_pad_group_v2::Event::Strip { strip } => {
                group.strips.push(Strip {
                    proxy: strip,
                    motion: Motion::default(),
                });
                info.strips = group.strips.len();
            }
            zwp_tablet_pad_group_v2::Event::Dial { dial } => {
                group.dials.push(Dial {
                    proxy: dial,
                    value120: 0,
                });
                info.dials = group.dials.len();
            }
            zwp_tablet_pad_group_v2::Event::Modes { modes } => info.modes = modes,
            zwp_tablet_pad_group_v2::Event::ModeSwitch { time, mode, .. } => {
                info.mode = mode;
                self.emit(
                    PadEvent::Mode {
                        group: index,
                        mode,
                        time: millis(time),
                    },
                    out,
                );
            }
            _ => {}
        }
    }

    pub(crate) fn handle_ring(
        &mut self,
        proxy: &ZwpTabletPadRingV2,
        event: zwp_tablet_pad_ring_v2::Event,
        out: &mut Vec<SeatEvent>,
    ) {
        let Some((group, index, ring)) =
            self.groups.iter_mut().enumerate().find_map(|(at, group)| {
                group
                    .rings
                    .iter_mut()
                    .enumerate()
                    .find(|(_, ring)| ring.proxy == *proxy)
                    .map(|(index, ring)| (at, index, ring))
            })
        else {
            return;
        };
        match event {
            zwp_tablet_pad_ring_v2::Event::Source { source } => {
                ring.motion.finger = matches!(source, Ok(zwp_tablet_pad_ring_v2::Source::Finger));
            }
            zwp_tablet_pad_ring_v2::Event::Angle { degrees } => {
                ring.motion.value = Some(degrees as f32);
            }
            zwp_tablet_pad_ring_v2::Event::Stop => ring.motion.stopped = true,
            zwp_tablet_pad_ring_v2::Event::Frame { time } => {
                if let Some((degrees, finger)) = ring.motion.take() {
                    self.emit(
                        PadEvent::Ring {
                            group,
                            ring: index,
                            degrees,
                            finger,
                            time: millis(time),
                        },
                        out,
                    );
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_strip(
        &mut self,
        proxy: &ZwpTabletPadStripV2,
        event: zwp_tablet_pad_strip_v2::Event,
        out: &mut Vec<SeatEvent>,
    ) {
        let Some((group, index, strip)) =
            self.groups.iter_mut().enumerate().find_map(|(at, group)| {
                group
                    .strips
                    .iter_mut()
                    .enumerate()
                    .find(|(_, strip)| strip.proxy == *proxy)
                    .map(|(index, strip)| (at, index, strip))
            })
        else {
            return;
        };
        match event {
            zwp_tablet_pad_strip_v2::Event::Source { source } => {
                strip.motion.finger = matches!(source, Ok(zwp_tablet_pad_strip_v2::Source::Finger));
            }
            zwp_tablet_pad_strip_v2::Event::Position { position } => {
                strip.motion.value = Some((position as f32 / STRIP_RANGE).clamp(0.0, 1.0));
            }
            zwp_tablet_pad_strip_v2::Event::Stop => strip.motion.stopped = true,
            zwp_tablet_pad_strip_v2::Event::Frame { time } => {
                if let Some((position, finger)) = strip.motion.take() {
                    self.emit(
                        PadEvent::Strip {
                            group,
                            strip: index,
                            position,
                            finger,
                            time: millis(time),
                        },
                        out,
                    );
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_dial(
        &mut self,
        proxy: &ZwpTabletPadDialV2,
        event: zwp_tablet_pad_dial_v2::Event,
        out: &mut Vec<SeatEvent>,
    ) {
        let Some((group, index, dial)) =
            self.groups.iter_mut().enumerate().find_map(|(at, group)| {
                group
                    .dials
                    .iter_mut()
                    .enumerate()
                    .find(|(_, dial)| dial.proxy == *proxy)
                    .map(|(index, dial)| (at, index, dial))
            })
        else {
            return;
        };
        match event {
            zwp_tablet_pad_dial_v2::Event::Delta { value120 } => dial.value120 += value120,
            zwp_tablet_pad_dial_v2::Event::Frame { time } => {
                let value120 = std::mem::take(&mut dial.value120);
                if value120 != 0 {
                    self.emit(
                        PadEvent::Dial {
                            group,
                            dial: index,
                            value120,
                            time: millis(time),
                        },
                        out,
                    );
                }
            }
            _ => {}
        }
    }
}

fn millis(time: u32) -> Duration {
    Duration::from_millis(u64::from(time))
}

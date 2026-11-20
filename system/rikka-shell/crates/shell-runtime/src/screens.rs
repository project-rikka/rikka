use rikka_types::{Point, Size};
use rikka_wayland_client::wl_output::{self, WlOutput};
use rikka_wayland_client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use rikka_wayland_client::zxdg_output_v1::{self, ZxdgOutputV1};
use rikka_wayland_client::{ObjectId, Proxy};

use shell_kit::Placement;

const DESCRIBED_BY_DONE: u32 = 2;
const RELEASABLE: u32 = 3;
const NAMED: u32 = 4;
const XDG_DONE_RETIRED: u32 = 3;

#[derive(Clone, Debug, Default)]
struct Description {
    location: (i32, i32),
    scale: i32,
    logical_position: Option<(i32, i32)>,
    logical_size: Option<(i32, i32)>,
    name: Option<String>,
}

impl Description {
    fn fresh() -> Description {
        Description {
            scale: 1,
            ..Description::default()
        }
    }

    fn placement(&self) -> Placement {
        let (x, y) = self.logical_position.unwrap_or(self.location);
        Placement {
            name: self.name.clone().unwrap_or_default(),
            position: Point::new(x as f32, y as f32),
            size: self.logical_size.map_or(Size::ZERO, |(width, height)| {
                Size::new(width as f32, height as f32)
            }),
            scale: self.scale.max(1) as f32,
        }
    }
}

#[derive(Debug)]
struct Screen {
    global: u32,
    output: WlOutput,
    xdg: Option<ZxdgOutputV1>,
    current: Option<Description>,
    pending: Description,
    pending_wl: bool,
    pending_xdg: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct OutputChange {
    pub(crate) settled: bool,
    pub(super) rescaled: bool,
}

#[derive(Debug, Default)]
pub(super) struct Screens {
    xdg: Option<ZxdgOutputManagerV1>,
    screens: Vec<Screen>,
}

impl Screens {
    pub(crate) fn new(xdg: Option<ZxdgOutputManagerV1>) -> Screens {
        Screens {
            xdg,
            screens: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, global: u32, output: WlOutput) {
        let xdg = self
            .xdg
            .as_ref()
            .map(|manager| manager.get_xdg_output(&output));
        self.screens.push(Screen {
            global,
            current: (output.version() < DESCRIBED_BY_DONE).then(Description::fresh),
            output,
            pending_xdg: xdg.is_some(),
            xdg,
            pending: Description::fresh(),
            pending_wl: true,
        });
    }

    pub(crate) fn remove(&mut self, global: u32) -> bool {
        let Some(at) = self
            .screens
            .iter()
            .position(|screen| screen.global == global)
        else {
            return false;
        };
        let screen = self.screens.remove(at);
        if let Some(xdg) = &screen.xdg {
            xdg.destroy();
        }
        if screen.output.version() >= RELEASABLE {
            screen.output.release();
        }
        true
    }

    pub(crate) fn outputs(&self) -> Vec<WlOutput> {
        self.screens
            .iter()
            .map(|screen| screen.output.clone())
            .collect()
    }

    pub(crate) fn placement(&self, output: &WlOutput) -> Option<Placement> {
        self.find(output)?
            .current
            .as_ref()
            .map(Description::placement)
    }

    pub(super) fn placement_of(&self, output: &ObjectId) -> Option<Placement> {
        self.screens
            .iter()
            .find(|screen| screen.output.object_id() == *output)?
            .current
            .as_ref()
            .map(Description::placement)
    }

    pub(crate) fn named(&self, name: &str) -> Option<&WlOutput> {
        self.screens
            .iter()
            .find(|screen| {
                screen
                    .current
                    .as_ref()
                    .and_then(|current| current.name.as_deref())
                    == Some(name)
            })
            .map(|screen| &screen.output)
    }

    pub(crate) fn scale(&self, output: &WlOutput) -> i32 {
        self.find(output)
            .and_then(|screen| screen.current.as_ref())
            .map_or(1, |current| current.scale)
    }

    pub(crate) fn incoming(&mut self, output: &WlOutput, event: wl_output::Event) -> OutputChange {
        let Some(screen) = self.find_mut(output) else {
            tracing::warn!("wl_output event for removed output: {event:?}");
            return OutputChange::default();
        };
        match event {
            wl_output::Event::Geometry { x, y, .. } => screen.pending.location = (x, y),
            wl_output::Event::Scale { factor } => screen.pending.scale = factor,
            wl_output::Event::Name { name } => screen.pending.name = Some(name),
            wl_output::Event::Done => return screen.done(),
            _ => {}
        }
        screen.pending_wl = true;
        OutputChange::default()
    }

    pub(super) fn handle_xdg_output(
        &mut self,
        xdg: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
    ) -> OutputChange {
        let Some(screen) = self
            .screens
            .iter_mut()
            .find(|screen| screen.xdg.as_ref() == Some(xdg))
        else {
            tracing::warn!("xdg_output event for removed output: {event:?}");
            return OutputChange::default();
        };
        let retired_done = xdg.version() >= XDG_DONE_RETIRED;
        if retired_done {
            screen.pending_xdg = false;
        }
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                screen.pending.logical_position = Some((x, y));
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                screen.pending.logical_size = Some((width, height));
            }
            zxdg_output_v1::Event::Name { name } => {
                if screen.output.version() < NAMED {
                    screen.pending.name = Some(name);
                }
            }
            zxdg_output_v1::Event::Done if retired_done => return OutputChange::default(),
            zxdg_output_v1::Event::Done => return screen.xdg_done(),
            _ => {}
        }
        if !retired_done {
            screen.pending_xdg = true;
        }
        OutputChange::default()
    }

    fn find(&self, output: &WlOutput) -> Option<&Screen> {
        self.screens.iter().find(|screen| &screen.output == output)
    }

    fn find_mut(&mut self, output: &WlOutput) -> Option<&mut Screen> {
        self.screens
            .iter_mut()
            .find(|screen| &screen.output == output)
    }
}

impl Screen {
    fn done(&mut self) -> OutputChange {
        let before = self.current.as_ref().map_or(1, |current| current.scale);
        self.current = Some(self.pending.clone());
        self.pending_wl = false;
        OutputChange {
            settled: !self.pending_xdg,
            rescaled: before != self.pending.scale,
        }
    }

    fn xdg_done(&mut self) -> OutputChange {
        self.current = Some(self.pending.clone());
        self.pending_xdg = false;
        OutputChange {
            settled: !self.pending_wl,
            rescaled: false,
        }
    }
}

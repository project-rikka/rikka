use rikka_wayland_client::wl_output::{self, Mode, WlOutput};
use rikka_wayland_client::wl_registry::WlRegistry;

const OUTPUT: u32 = 4;

const RELEASES: u32 = 3;

#[derive(Debug)]
struct Screen {
    global: u32,
    output: WlOutput,
    scale: u32,
    refresh: Option<u32>,
}

#[derive(Debug, Default)]
pub(super) struct Screens {
    screens: Vec<Screen>,
}

impl Screens {
    pub(super) fn add(&mut self, registry: &WlRegistry, name: u32, version: u32) {
        self.screens.push(Screen {
            global: name,
            output: registry.bind(name, version.min(OUTPUT)),
            scale: 1,
            refresh: None,
        });
    }

    pub(super) fn remove(&mut self, name: u32) -> Option<WlOutput> {
        let at = self
            .screens
            .iter()
            .position(|screen| screen.global == name)?;
        let screen = self.screens.remove(at);
        if screen.output.version() >= RELEASES {
            screen.output.release();
        }
        Some(screen.output)
    }

    pub(super) fn handle_event(&mut self, output: &WlOutput, event: wl_output::Event) -> bool {
        let Some(screen) = self
            .screens
            .iter_mut()
            .find(|screen| &screen.output == output)
        else {
            return false;
        };
        match event {
            wl_output::Event::Mode { flags, refresh, .. } if flags.contains(Mode::CURRENT) => {
                let refresh = u32::try_from(refresh).ok().filter(|refresh| *refresh > 0);
                std::mem::replace(&mut screen.refresh, refresh) != refresh
            }
            wl_output::Event::Scale { factor } => {
                let scale = u32::try_from(factor).unwrap_or(1).max(1);
                std::mem::replace(&mut screen.scale, scale) != scale
            }
            _ => false,
        }
    }

    pub(super) fn scale_of(&self, shown_on: &[WlOutput]) -> u32 {
        self.shown(shown_on)
            .map(|screen| screen.scale)
            .max()
            .unwrap_or(1)
    }

    pub(super) fn refresh_of(&self, shown_on: &[WlOutput]) -> Option<u32> {
        self.shown(shown_on)
            .filter_map(|screen| screen.refresh)
            .max()
            .or_else(|| {
                self.screens
                    .iter()
                    .filter_map(|screen| screen.refresh)
                    .max()
            })
    }

    fn shown<'a>(&'a self, shown_on: &'a [WlOutput]) -> impl Iterator<Item = &'a Screen> {
        self.screens
            .iter()
            .filter(move |screen| shown_on.contains(&screen.output))
    }
}

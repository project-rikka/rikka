use rikka_picture::RefreshRate;
use rikka_wayland::output_scale;
use rikka_wayland_client::wl_output::{self, Mode, WlOutput};
use rikka_wayland_client::wl_registry::WlRegistry;
use rikka_wayland_client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use rikka_wayland_client::zxdg_output_v1::{self, ZxdgOutputV1};

const OUTPUT: u32 = 4;

const RELEASES: u32 = 3;

#[derive(Debug)]
struct Output {
    global: u32,
    output: WlOutput,
    xdg: Option<ZxdgOutputV1>,
    refresh: Option<RefreshRate>,
    scale: u32,
    mode: Option<(i32, i32)>,
    logical: Option<(i32, i32)>,
}

impl Output {
    fn fine_scale(&self) -> Option<f64> {
        output_scale(self.mode?, self.logical?)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Outputs {
    outputs: Vec<Output>,
}

impl Outputs {
    pub(crate) fn add(
        &mut self,
        registry: &WlRegistry,
        (name, version): (u32, u32),
        xdg_output_manager: Option<&ZxdgOutputManagerV1>,
    ) {
        let output = registry.bind(name, version.min(OUTPUT));
        let xdg = xdg_output_manager.map(|manager| manager.get_xdg_output(&output).with_key(name));
        self.outputs.push(Output {
            global: name,
            output,
            xdg,
            refresh: None,
            scale: 1,
            mode: None,
            logical: None,
        });
    }

    pub(crate) fn remove(&mut self, name: u32) -> Option<WlOutput> {
        let at = self
            .outputs
            .iter()
            .position(|output| output.global == name)?;
        let removed = self.outputs.remove(at);
        if let Some(xdg) = removed.xdg {
            xdg.destroy();
        }
        if removed.output.version() >= RELEASES {
            removed.output.release();
        }
        Some(removed.output)
    }

    pub(crate) fn handle_event(&mut self, output: &WlOutput, event: wl_output::Event) -> bool {
        let Some(known) = self
            .outputs
            .iter_mut()
            .find(|known| &known.output == output)
        else {
            return false;
        };
        match event {
            wl_output::Event::Mode {
                flags,
                width,
                height,
                refresh,
            } if flags.contains(Mode::CURRENT) => {
                known.mode = Some((width, height));
                let refresh = u32::try_from(refresh)
                    .ok()
                    .and_then(RefreshRate::from_millihertz);
                std::mem::replace(&mut known.refresh, refresh) != refresh
            }
            wl_output::Event::Scale { factor } => {
                let scale = u32::try_from(factor).unwrap_or(1).max(1);
                std::mem::replace(&mut known.scale, scale) != scale
            }
            _ => false,
        }
    }

    pub(crate) fn handle_xdg_event(&mut self, global: u32, event: zxdg_output_v1::Event) {
        if let zxdg_output_v1::Event::LogicalSize { width, height } = event
            && let Some(known) = self.outputs.iter_mut().find(|known| known.global == global)
        {
            known.logical = Some((width, height));
        }
    }

    pub(crate) fn scale_of<'a>(&self, on: impl IntoIterator<Item = &'a WlOutput>) -> u32 {
        on.into_iter()
            .filter_map(|output| self.find(output))
            .map(|output| output.scale)
            .max()
            .unwrap_or(1)
    }

    pub(crate) fn expected_scale(&self) -> Option<f64> {
        let mut scales = self.outputs.iter().map(Output::fine_scale);
        let first = scales.next()??;
        scales.all(|scale| scale == Some(first)).then_some(first)
    }

    pub(crate) fn publish<'a>(&self, shown_on: impl IntoIterator<Item = &'a WlOutput>) {
        let fastest_shown = shown_on
            .into_iter()
            .filter_map(|output| self.find(output)?.refresh)
            .max();
        let fastest = fastest_shown.or_else(|| {
            self.outputs
                .iter()
                .filter_map(|output| output.refresh)
                .max()
        });
        if let Some(rate) = fastest {
            rate.publish();
        }
    }

    fn find(&self, output: &WlOutput) -> Option<&Output> {
        self.outputs.iter().find(|known| &known.output == output)
    }
}

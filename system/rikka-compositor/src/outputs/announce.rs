use rikka_compositor_geometry::Size;
use rikka_compositor_output::{
    Millimetres, Mode, OutputHost, OutputInfo, Outputs as WaylandOutputs, Publication, Subpixel,
};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_types::{OutputId, OutputLayout};

use super::{Output, Outputs};
use crate::context::capture_ctx;
use crate::context::windows_ctx;
use crate::state::Compositor;

fn info(output: &Output) -> Option<OutputInfo> {
    let config = output.enabled_config()?;
    let head = output.head();
    Some(OutputInfo {
        description: format!("{} {} ({})", head.make, head.model, head.name),
        make: head.make.clone(),
        model: head.model.clone(),
        serial: head.serial.clone(),
        physical_size: Millimetres {
            width: head.size_mm.0 as i32,
            height: head.size_mm.1 as i32,
        },
        subpixel: Subpixel::Unknown,
        transform: config.transform,
        mode: Mode {
            size: Size::new(config.mode.width as i32, config.mode.height as i32),
            refresh_millihertz: config.mode.refresh as i32,
        },
        scale: config.scale,
        position: config.position,
    })
}

impl Outputs {
    fn unlit(&self) -> Vec<OutputId> {
        self.wayland
            .connected()
            .filter(|output| {
                self.all
                    .get(output)
                    .is_none_or(|lit| lit.enabled_config().is_none())
            })
            .collect()
    }
}

impl OutputHost for Compositor {
    fn outputs(&mut self) -> &mut WaylandOutputs {
        &mut self.outputs.wayland
    }

    fn output_bound(&mut self, output: OutputId, resource: &WlOutput) {
        self.presence.bound(output, resource, &mut self.surfaces);
        self.workspace_list.output_bound(output, resource);
    }
}

impl Compositor {
    pub(crate) fn announce_outputs(&mut self) {
        let followed: Vec<(OutputId, OutputLayout)> = self
            .windows
            .desktop
            .outputs()
            .map(|(id, output)| (id, output.layout()))
            .collect();
        for output in self.outputs.unlit() {
            if self.outputs.wayland.withdraw(output) {
                capture_ctx!(self).forget_captures_of(output);
                self.presence.forget_output(output, &mut self.surfaces);
                self.outputs.colors.forget(output);
                self.surfaces.forget_output(output);
            }
            if !self.outputs.all.contains_key(&output) {
                self.outputs.wayland.disconnect(output);
            }
        }
        let lit: Vec<(OutputId, OutputInfo)> = self
            .outputs
            .all
            .values()
            .filter_map(|output| Some((output.id(), info(output)?)))
            .collect();
        let shown: Vec<OutputId> = lit.iter().map(|(output, _)| *output).collect();
        for (output, info) in lit {
            let Outputs {
                all,
                wayland,
                colors,
            } = &mut self.outputs;
            let recolored = all
                .get(&output)
                .and_then(|lit| Some((lit.head(), lit.enabled_config()?)))
                .is_some_and(|(head, config)| {
                    colors.announce(output, (head, &config), &mut self.surfaces)
                });
            match wayland.publish(&self.core.display, output, info) {
                Publication::Changed => capture_ctx!(self).capture_constraints_changed(output),
                Publication::Unchanged if recolored => self.outputs.wayland.done(output),
                Publication::Published | Publication::Unchanged => {}
            }
        }
        windows_ctx!(self).windows_follow_displays(&followed, &shown);
        self.windows.session_changed();
    }
}

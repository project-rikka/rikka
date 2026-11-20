mod anchor;
mod announce;
mod arrangement;
mod config;
mod head;
mod power;

use std::collections::BTreeMap;

use rikka_compositor_geometry::{Logical, Physical, Point, Rect, Size};
use rikka_compositor_input::{PointerRestrictions, PointerRoute, RouteOutput};
use rikka_compositor_output::Outputs as WaylandOutputs;
use rikka_compositor_types::OutputId;

use crate::color::AnnouncedColors;

pub(crate) use config::OutputConfig;
pub(crate) use head::{FALLBACK_REFRESH, Head, Mode, interval};

#[derive(Debug, Clone)]
pub(crate) struct Output {
    id: OutputId,
    head: Head,
    asked: Option<OutputConfig>,
    config: Option<OutputConfig>,
    cursor_plane: bool,
}

impl Output {
    pub(crate) fn id(&self) -> OutputId {
        self.id
    }

    pub(crate) fn head(&self) -> &Head {
        &self.head
    }

    pub(crate) fn name(&self) -> &str {
        &self.head.name
    }

    pub(crate) fn asked(&self) -> Option<OutputConfig> {
        self.asked
    }

    pub(crate) fn enabled_config(&self) -> Option<OutputConfig> {
        self.config.filter(|config| config.enabled)
    }

    pub(crate) fn area(&self) -> Option<Rect<i32, Logical>> {
        let config = self.enabled_config()?;
        let size = self.physical_size()?;
        let logical = Size::new(
            (f64::from(size.w) / config.scale).round() as i32,
            (f64::from(size.h) / config.scale).round() as i32,
        );
        Some(Rect::new(config.position, logical))
    }

    pub(crate) fn physical_size(&self) -> Option<Size<i32, Physical>> {
        let config = self.enabled_config()?;
        let mode = Size::new(config.mode.width as i32, config.mode.height as i32);
        Some(mode.transformed(config.transform))
    }

    pub(crate) fn scale(&self) -> f64 {
        self.enabled_config().map_or(1.0, |config| config.scale)
    }

    pub(crate) fn has_cursor_plane(&self) -> bool {
        self.cursor_plane
    }
}

#[derive(Debug, Default)]
pub(crate) struct Outputs {
    all: BTreeMap<OutputId, Output>,
    wayland: WaylandOutputs,
    colors: AnnouncedColors,
}

impl Outputs {
    pub(crate) fn connect(&mut self, head: Head) -> OutputId {
        let id = self.wayland.connect(head.name.as_str());
        tracing::info!("{} connected as {id:?}", head.name);
        self.all.insert(
            id,
            Output {
                id,
                head,
                asked: None,
                config: None,
                cursor_plane: false,
            },
        );
        id
    }

    pub(crate) fn disconnect(&mut self, id: OutputId) {
        if let Some(output) = self.all.remove(&id) {
            tracing::info!("{} disconnected", output.head.name);
        }
    }

    pub(crate) fn describe(&mut self, id: OutputId, head: Head) {
        if let Some(output) = self.all.get_mut(&id) {
            output.head = head;
        }
    }

    pub(crate) fn ask(&mut self, id: OutputId, config: OutputConfig) {
        if let Some(output) = self.all.get_mut(&id) {
            output.asked = Some(config);
        }
    }

    pub(crate) fn apply(&mut self, id: OutputId, config: OutputConfig) {
        if let Some(output) = self.all.get_mut(&id) {
            output.config = Some(config);
        }
    }

    pub(crate) fn set_cursor_plane(&mut self, id: OutputId, available: bool) {
        if let Some(output) = self.all.get_mut(&id) {
            output.cursor_plane = available;
        }
    }

    pub(crate) fn get(&self, id: OutputId) -> Option<&Output> {
        self.all.get(&id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Output> {
        self.all.values()
    }

    pub(crate) fn enabled(&self) -> impl Iterator<Item = &Output> {
        self.all
            .values()
            .filter(|output| output.enabled_config().is_some())
    }

    pub(crate) fn route(
        &self,
        hardware_cursor: bool,
        restrictions: PointerRestrictions,
    ) -> PointerRoute {
        PointerRoute {
            outputs: self
                .enabled()
                .filter_map(|output| {
                    let area = output.area()?;
                    let config = output.enabled_config()?;
                    let cursor = (hardware_cursor && output.cursor_plane).then_some(output.id);
                    Some(RouteOutput {
                        area,
                        scale: config.scale,
                        cursor,
                    })
                })
                .collect(),
            restrictions,
        }
    }

    pub(crate) fn wayland(&self) -> &WaylandOutputs {
        &self.wayland
    }

    pub(crate) fn containing(&self, point: Point<f64, Logical>) -> Option<&Output> {
        self.enabled().find(|output| {
            output
                .area()
                .is_some_and(|area| area.to_f64().contains(point))
        })
    }
}

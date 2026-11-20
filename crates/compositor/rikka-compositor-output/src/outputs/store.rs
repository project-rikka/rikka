use std::fmt;

use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::zxdg_output_v1::ZxdgOutputV1;
use rikka_compositor_server::ByClient;
use rikka_compositor_server::{DisplayHandle, Global};
use rikka_compositor_types::{LayoutView, Layouts, OutputId};
use rikka_wayland_wire::{ClientId, Resource};
use slotmap::SlotMap;

use super::announce;
use super::info::OutputInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publication {
    Published,
    Changed,
    Unchanged,
}

pub struct Outputs {
    heads: SlotMap<OutputId, Head>,
    layouts: Layouts,
}

struct Head {
    name: String,
    published: Option<Published>,
}

struct Published {
    info: OutputInfo,
    global: Global,
    resources: ByClient<WlOutput>,
    xdg: Vec<(ZxdgOutputV1, WlOutput)>,
}

impl Outputs {
    pub fn new() -> Self {
        Self {
            heads: SlotMap::with_key(),
            layouts: Layouts::default(),
        }
    }

    pub fn connect(&mut self, name: impl Into<String>) -> OutputId {
        self.heads.insert(Head {
            name: name.into(),
            published: None,
        })
    }

    pub fn disconnect(&mut self, output: OutputId) -> bool {
        self.withdraw(output);
        self.heads.remove(output).is_some()
    }

    pub fn publish<S: 'static>(
        &mut self,
        display: &DisplayHandle<S>,
        output: OutputId,
        info: OutputInfo,
    ) -> Publication {
        let Some(head) = self.heads.get_mut(output) else {
            return Publication::Unchanged;
        };
        self.layouts.set(output, info.layout());
        let Some(published) = &mut head.published else {
            head.published = Some(Published {
                global: display.global::<WlOutput>().key(output).publish(),
                info,
                resources: ByClient::default(),
                xdg: Vec::new(),
            });
            return Publication::Published;
        };
        if published.info == info {
            return Publication::Unchanged;
        }
        let old = std::mem::replace(&mut published.info, info);
        let new = &published.info;
        let mut pending_done: Vec<WlOutput> = Vec::new();
        for resource in published
            .resources
            .all()
            .filter(|resource| resource.alive())
        {
            if announce::wl_changes(resource, &old, new) {
                pending_done.push(resource.clone());
            }
        }
        for (xdg, resource) in published.xdg.iter().filter(|(xdg, _)| xdg.alive()) {
            if announce::xdg_changes(xdg, &old, new) && !pending_done.contains(resource) {
                pending_done.push(resource.clone());
            }
        }
        for resource in pending_done {
            resource.done();
        }
        Publication::Changed
    }

    pub fn withdraw(&mut self, output: OutputId) -> bool {
        let Some(published) = self
            .heads
            .get_mut(output)
            .and_then(|head| head.published.take())
        else {
            return false;
        };
        published.global.remove();
        self.layouts.remove(output);
        true
    }

    pub fn layouts(&self) -> LayoutView {
        self.layouts.view()
    }

    pub fn done(&self, output: OutputId) {
        let Some(published) = self.published_entry(output) else {
            return;
        };
        for resource in published
            .resources
            .all()
            .filter(|resource| resource.alive())
        {
            resource.done();
        }
    }

    pub fn info(&self, output: OutputId) -> Option<&OutputInfo> {
        self.published_entry(output)
            .map(|published| &published.info)
    }

    pub fn name(&self, output: OutputId) -> Option<&str> {
        self.heads.get(output).map(|head| head.name.as_str())
    }

    pub fn is_published(&self, output: OutputId) -> bool {
        self.published_entry(output).is_some()
    }

    pub fn connected(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.heads.keys()
    }

    pub fn published(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.heads
            .iter()
            .filter(|(_, head)| head.published.is_some())
            .map(|(output, _)| output)
    }

    pub fn named(&self, name: &str) -> Option<OutputId> {
        self.heads
            .iter()
            .find(|(_, head)| head.published.is_some() && head.name == name)
            .map(|(output, _)| output)
    }

    pub fn resources(
        &self,
        output: OutputId,
        client: ClientId,
    ) -> impl Iterator<Item = WlOutput> + '_ {
        self.published_entry(output)
            .map_or(&[][..], |published| published.resources.for_client(client))
            .iter()
            .filter(|resource| resource.alive())
            .cloned()
    }

    pub fn output_of(&self, resource: &WlOutput) -> Option<OutputId> {
        let output = resource.key::<OutputId>()?;
        self.published_entry(output)?
            .resources
            .for_client(resource.object().client())
            .contains(resource)
            .then_some(output)
    }

    pub(super) fn bind(&mut self, output: OutputId, resource: &WlOutput) -> bool {
        let Some(head) = self.heads.get_mut(output) else {
            return false;
        };
        let Some(published) = &mut head.published else {
            return false;
        };
        announce::wl_burst(resource, &head.name, &published.info);
        published.resources.add(resource.clone());
        true
    }

    pub(super) fn attach_xdg(&mut self, output: OutputId, xdg: &ZxdgOutputV1, resource: &WlOutput) {
        let Some(head) = self.heads.get_mut(output) else {
            return;
        };
        let Some(published) = &mut head.published else {
            return;
        };
        announce::xdg_burst(xdg, resource, &head.name, &published.info);
        published.xdg.push((xdg.clone(), resource.clone()));
    }

    pub(super) fn forget(&mut self, output: OutputId, resource: &WlOutput) {
        if let Some(published) = self.published_entry_mut(output) {
            published.resources.remove(resource);
        }
    }

    pub(super) fn forget_xdg(&mut self, output: OutputId, xdg: &ZxdgOutputV1) {
        if let Some(published) = self.published_entry_mut(output) {
            published.xdg.retain(|(xdg_output, _)| xdg_output != xdg);
        }
    }

    fn published_entry(&self, output: OutputId) -> Option<&Published> {
        self.heads.get(output)?.published.as_ref()
    }

    fn published_entry_mut(&mut self, output: OutputId) -> Option<&mut Published> {
        self.heads.get_mut(output)?.published.as_mut()
    }
}

impl Default for Outputs {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Outputs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_map()
            .entries(self.heads.iter().map(|(id, head)| {
                let bound = head
                    .published
                    .as_ref()
                    .map(|published| published.resources.all().count());
                (id, (&head.name, bound))
            }))
            .finish()
    }
}

use rikka_compositor_protocols::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use rikka_compositor_protocols::ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1};
use rikka_compositor_server::Client;
use slotmap::SlotMap;

use crate::host::ShellHost;
use crate::ids::{ForeignToplevelId, ToplevelId};
use crate::shell::Shell;
use crate::xdg::Toplevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignToplevel<'a> {
    Xdg(ToplevelId),
    Named {
        title: Option<&'a str>,
        app_id: Option<&'a str>,
    },
}

#[derive(Debug)]
struct Entry {
    identifier: String,
    title: Option<String>,
    app_id: Option<String>,
    handles: Vec<ExtForeignToplevelHandleV1>,
}

#[derive(Debug, Default)]
pub(crate) struct Published {
    lists: Vec<ExtForeignToplevelListV1>,
    toplevels: SlotMap<ForeignToplevelId, Entry>,
}

impl Shell {
    pub fn publish_toplevel(&mut self, info: ForeignToplevel<'_>) -> ForeignToplevelId {
        let (title, app_id) = names(&self.toplevels, info);
        let published = &mut self.published;
        let id = published.toplevels.insert(Entry {
            identifier: rikka_compositor_types::unguessable(),
            title: title.map(str::to_owned),
            app_id: app_id.map(str::to_owned),
            handles: Vec::new(),
        });
        published.lists.retain(ExtForeignToplevelListV1::alive);
        if let Some(entry) = published.toplevels.get_mut(id) {
            for list in &published.lists {
                announce(list, id, entry);
            }
        }
        id
    }

    pub fn update_published_toplevel(
        &mut self,
        toplevel: ForeignToplevelId,
        info: ForeignToplevel<'_>,
    ) {
        let (title, app_id) = names(&self.toplevels, info);
        let Some(entry) = self.published.toplevels.get_mut(toplevel) else {
            return;
        };
        let title = renamed(&mut entry.title, title);
        let app_id = renamed(&mut entry.app_id, app_id);
        if !(title || app_id) {
            return;
        }
        entry.handles.retain(ExtForeignToplevelHandleV1::alive);
        for handle in &entry.handles {
            if title && let Some(title) = &entry.title {
                handle.title(title);
            }
            if app_id && let Some(app_id) = &entry.app_id {
                handle.app_id(app_id);
            }
            handle.done();
        }
    }

    pub fn close_published_toplevel(&mut self, toplevel: ForeignToplevelId) {
        let Some(entry) = self.published.toplevels.remove(toplevel) else {
            return;
        };
        for handle in entry.handles {
            handle.closed();
        }
    }

    pub fn published_identifier(&self, toplevel: ForeignToplevelId) -> Option<&str> {
        self.published
            .toplevels
            .get(toplevel)
            .map(|entry| entry.identifier.as_str())
    }

    pub fn published_toplevel_of(
        &self,
        handle: &ExtForeignToplevelHandleV1,
    ) -> Option<ForeignToplevelId> {
        handle
            .key::<ForeignToplevelId>()
            .filter(|id| self.published.toplevels.contains_key(*id))
    }
}

pub(crate) fn bound<S: ShellHost>(state: &mut S, list: ExtForeignToplevelListV1) {
    let published = &mut state.shell().published;
    for (id, entry) in &mut published.toplevels {
        announce(&list, id, entry);
    }
    published.lists.push(list);
}

pub(crate) fn list<S: ShellHost>(
    state: &mut S,
    list: ExtForeignToplevelListV1,
    request: ext_foreign_toplevel_list_v1::Request,
) {
    match request {
        ext_foreign_toplevel_list_v1::Request::Stop => {
            forget(state, &list);
            list.finished();
        }
        ext_foreign_toplevel_list_v1::Request::Destroy => {}
    }
}

pub(crate) fn list_destroyed<S: ShellHost>(state: &mut S, list: ExtForeignToplevelListV1) {
    forget(state, &list);
}

pub(crate) fn handle<S>(
    _state: &mut S,
    _handle: ExtForeignToplevelHandleV1,
    _request: ext_foreign_toplevel_handle_v1::Request,
) {
}

pub(crate) fn handle_destroyed<S: ShellHost>(state: &mut S, handle: ExtForeignToplevelHandleV1) {
    let Some(id) = handle.key::<ForeignToplevelId>() else {
        return;
    };
    if let Some(entry) = state.shell().published.toplevels.get_mut(id) {
        entry.handles.retain(|known| *known != handle);
    }
}

fn forget<S: ShellHost>(state: &mut S, list: &ExtForeignToplevelListV1) {
    state.shell().published.lists.retain(|known| known != list);
}

fn announce(list: &ExtForeignToplevelListV1, id: ForeignToplevelId, entry: &mut Entry) {
    let Some(client) = Client::from_resource(list) else {
        return;
    };
    let handle = client.create::<ExtForeignToplevelHandleV1>(list.version());
    handle.set_key(id);
    list.toplevel(&handle);
    handle.identifier(&entry.identifier);
    if let Some(title) = &entry.title {
        handle.title(title);
    }
    if let Some(app_id) = &entry.app_id {
        handle.app_id(app_id);
    }
    handle.done();
    entry.handles.push(handle);
}

fn names<'a>(
    toplevels: &'a SlotMap<ToplevelId, Toplevel>,
    info: ForeignToplevel<'a>,
) -> (Option<&'a str>, Option<&'a str>) {
    match info {
        ForeignToplevel::Xdg(toplevel) => {
            toplevels.get(toplevel).map_or((None, None), |toplevel| {
                (toplevel.title(), toplevel.app_id())
            })
        }
        ForeignToplevel::Named { title, app_id } => (title, app_id),
    }
}

fn renamed(field: &mut Option<String>, value: Option<&str>) -> bool {
    if field.as_deref() == value {
        return false;
    }
    match (field.as_mut(), value) {
        (Some(current), Some(value)) => {
            current.clear();
            current.push_str(value);
        }
        (_, value) => *field = value.map(str::to_owned),
    }
    true
}

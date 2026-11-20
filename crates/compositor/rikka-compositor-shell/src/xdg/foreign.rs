use std::collections::HashMap;

use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_protocols::zxdg_exported_v2::{self, ZxdgExportedV2};
use rikka_compositor_protocols::zxdg_exporter_v2::{self, ZxdgExporterV2};
use rikka_compositor_protocols::zxdg_imported_v2::{self, ZxdgImportedV2};
use rikka_compositor_protocols::zxdg_importer_v2::{self, ZxdgImporterV2};
use slotmap::SlotMap;

use super::toplevel_lifecycle;
use super::xdg_role::{XdgObject, XdgRole};
use crate::host::ShellHost;
use crate::ids::{ExportId, ImportId, ToplevelId};

#[derive(Debug, Default)]
pub(crate) struct Foreign {
    exports: HashMap<String, ToplevelId>,
    exported: SlotMap<ExportId, String>,
    imports: SlotMap<ImportId, Import>,
}

#[derive(Debug)]
struct Import {
    resource: ZxdgImportedV2,
    handle: String,
    children: Vec<ToplevelId>,
}

pub(crate) fn exporter<S: ShellHost>(
    state: &mut S,
    exporter: ZxdgExporterV2,
    request: zxdg_exporter_v2::Request,
) {
    let zxdg_exporter_v2::Request::ExportToplevel { id, surface } = request else {
        return;
    };
    let Some(toplevel) = toplevel_behind(state, &surface) else {
        return exporter.post_error(zxdg_exporter_v2::Error::InvalidSurface);
    };
    let handle = rikka_compositor_types::unguessable();
    state
        .shell()
        .foreign
        .exports
        .insert(handle.clone(), toplevel);
    id.handle(&handle);
    let export = state.shell().foreign.exported.insert(handle);
    id.set_key(export);
}

pub(crate) fn exported<S>(
    _state: &mut S,
    _exported: ZxdgExportedV2,
    _request: zxdg_exported_v2::Request,
) {
}

pub(crate) fn exported_destroyed<S: ShellHost>(state: &mut S, exported: ZxdgExportedV2) {
    if let Some(handle) = exported
        .key::<ExportId>()
        .and_then(|export| state.shell().foreign.exported.remove(export))
    {
        revoke(state, &handle);
    }
}

pub(crate) fn importer<S: ShellHost>(
    state: &mut S,
    _importer: ZxdgImporterV2,
    request: zxdg_importer_v2::Request,
) {
    let zxdg_importer_v2::Request::ImportToplevel { id, handle } = request else {
        return;
    };
    let foreign = &mut state.shell().foreign;
    if !foreign.exports.contains_key(handle) {
        return id.destroyed();
    }
    let import = foreign.imports.insert(Import {
        resource: id.clone(),
        handle: handle.to_owned(),
        children: Vec::new(),
    });
    id.set_key(import);
}

pub(crate) fn imported<S: ShellHost>(
    state: &mut S,
    imported: ZxdgImportedV2,
    request: zxdg_imported_v2::Request,
) {
    let zxdg_imported_v2::Request::SetParentOf { surface } = request else {
        return;
    };
    let Some(child) = toplevel_behind(state, &surface) else {
        return imported.post_error(zxdg_imported_v2::Error::InvalidSurface);
    };
    let Some(key) = imported.key::<ImportId>() else {
        return;
    };
    let foreign = &state.shell().foreign;
    let Some(parent) = foreign
        .imports
        .get(key)
        .and_then(|import| foreign.exports.get(&import.handle))
        .copied()
    else {
        return;
    };
    if toplevel_lifecycle::set_parent(state, child, Some(parent))
        && let Some(import) = state.shell().foreign.imports.get_mut(key)
    {
        import.children.push(child);
    }
}

pub(crate) fn imported_destroyed<S: ShellHost>(state: &mut S, imported: ZxdgImportedV2) {
    let Some(key) = imported.key::<ImportId>() else {
        return;
    };
    let foreign = &mut state.shell().foreign;
    let Some(import) = foreign.imports.remove(key) else {
        return;
    };
    let Some(parent) = foreign.exports.get(&import.handle).copied() else {
        return;
    };
    release_children(state, &import.children, parent);
}

pub(crate) fn toplevel_gone<S: ShellHost>(state: &mut S, toplevel: ToplevelId) {
    let handles: Vec<String> = state
        .shell()
        .foreign
        .exports
        .iter()
        .filter(|(_, exported)| **exported == toplevel)
        .map(|(handle, _)| handle.clone())
        .collect();
    for handle in handles {
        revoke(state, &handle);
    }
}

fn revoke<S: ShellHost>(state: &mut S, handle: &str) {
    let foreign = &mut state.shell().foreign;
    let Some(parent) = foreign.exports.remove(handle) else {
        return;
    };
    let revoked: Vec<ImportId> = foreign
        .imports
        .iter()
        .filter(|(_, import)| import.handle == handle)
        .map(|(key, _)| key)
        .collect();
    for key in revoked {
        let Some(import) = state.shell().foreign.imports.remove(key) else {
            continue;
        };
        import.resource.destroyed();
        release_children(state, &import.children, parent);
    }
}

fn release_children<S: ShellHost>(state: &mut S, children: &[ToplevelId], parent: ToplevelId) {
    for child in children {
        let still_parented = state
            .shell()
            .toplevels
            .get(*child)
            .is_some_and(|record| record.parent == Some(parent));
        if still_parented {
            toplevel_lifecycle::set_parent(state, *child, None);
        }
    }
}

fn toplevel_behind<S: ShellHost>(state: &mut S, surface: &WlSurface) -> Option<ToplevelId> {
    let surface = state.surfaces().lookup(surface)?;
    match state.surfaces().role::<XdgRole>(surface)?.object? {
        XdgObject::Toplevel(toplevel) => Some(toplevel),
        XdgObject::Popup(_) => None,
    }
}

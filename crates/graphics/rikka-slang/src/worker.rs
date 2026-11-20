use core::ffi::c_void;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::com::{Blob, Com, Diagnostics};
use crate::compiler::{Optimization, Settings};
use crate::error::{Error, Result};
use crate::exit;
use crate::ffi::{
    self, CompilerOptionEntry, CompilerOptionValue, ComponentTypeVtbl, GlobalSessionDesc,
    GlobalSessionVtbl, ModuleVtbl, SessionDesc, SessionVtbl, TargetDesc,
};
use crate::file_system::Library;
use crate::layout::{EntryPoint, Parameter, TypeLayout};
use crate::program::Source;
use crate::reflect::{self, Layout, check};
use crate::spirv;

pub(crate) const MODULE_EXTENSION: &str = ".slang";

const LIBRARY_ROOT: &CStr = c".";

const RENEW_AFTER: u64 = 8;

const RECYCLE_AFTER: u64 = 2 * RENEW_AFTER;

const WARM_UP_MODULE: &str = "rikka-slang-warm-up";

const WARM_UP: &str = r#"
struct Parameters { float4 tint; }

[[vk::push_constant]] ConstantBuffer<Parameters> parameters;

[shader("vertex")]
float4 place(uint vertex : SV_VertexID) : SV_Position {
    return float4(float(vertex & 1), float(vertex >> 1), 0.0, 1.0);
}

[shader("fragment")]
float4 shade(float4 position : SV_Position) : SV_Target {
    return parameters.tint * position;
}
"#;

pub(crate) struct Worker {
    session: Com<SessionVtbl>,
    global: Com<GlobalSessionVtbl>,
    loaded: u64,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("loaded", &self.loaded)
            .finish_non_exhaustive()
    }
}

pub(crate) struct Reflection {
    pub parameters: Vec<Parameter>,
    pub types: Vec<(String, TypeLayout)>,
    pub conformances: Vec<(String, String, bool)>,
}

#[derive(Default)]
pub(crate) struct Generated {
    pub entry_points: Vec<(usize, EntryPoint)>,
    pub reflection: Option<Reflection>,
}

impl Worker {
    pub(crate) fn open(settings: &Arc<Settings>) -> Result<Self> {
        let global = global_session()?;
        let session = open_session(&global, settings)?;
        let mut worker = Self {
            session,
            global,
            loaded: 0,
        };
        exit::warmed(|| worker.warm_up())?;
        Ok(worker)
    }

    fn warm_up(&mut self) -> Result<()> {
        let settings = Arc::new(Settings {
            library: HashMap::new(),
            optimization: Optimization::High,
        });
        let session = open_session(&self.global, &settings)?;
        let session = std::mem::replace(&mut self.session, session);
        let source = Source::new(WARM_UP_MODULE, WARM_UP).flatten();
        let warmed = self.generate(&settings, &source, &AtomicUsize::new(0), true);
        self.session = session;
        self.loaded = 0;
        warmed.map(|_| ())
    }

    pub(crate) fn is_due(&self) -> bool {
        self.loaded >= RENEW_AFTER
    }

    pub(crate) fn renew(&mut self, settings: &Arc<Settings>, source: &Source<'_>) -> Result<()> {
        self.session = open_session(&self.global, settings)?;
        self.loaded = 0;
        self.load(source.name, source.text).map(|_| ())
    }

    pub(crate) fn reflect(
        &mut self,
        settings: &Arc<Settings>,
        source: &Source<'_>,
    ) -> Result<Reflection> {
        self.link(settings, source)?.reflection(source)
    }

    pub(crate) fn generate(
        &mut self,
        settings: &Arc<Settings>,
        source: &Source<'_>,
        next: &AtomicUsize,
        reflect: bool,
    ) -> Result<Generated> {
        let linked = self.link(settings, source)?;
        let mut entry_points = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            if index >= linked.count {
                break;
            }
            let entry = linked.entry_point(index)?;
            let entry = match source.flatten {
                true => spirv::flatten(entry)?,
                false => entry,
            };
            entry_points.push((index, entry));
        }
        let reflection = match reflect {
            true => Some(linked.reflection(source)?),
            false => None,
        };
        Ok(Generated {
            entry_points,
            reflection,
        })
    }

    fn link(&mut self, settings: &Arc<Settings>, source: &Source<'_>) -> Result<Linked> {
        if self.loaded >= RECYCLE_AFTER {
            self.session = open_session(&self.global, settings)?;
            self.loaded = 0;
        }
        let module = self.load(source.name, source.text)?;
        let mut diagnostics = Diagnostics::default();
        let entries = entry_points(module, &source.entries)?;
        let mut components: Vec<*mut c_void> = vec![module];
        components.extend(entries.iter().map(Com::raw));
        let composite = component(&mut diagnostics, |out, slot| unsafe {
            (self.session.vtable().create_composite_component_type)(
                self.session.raw(),
                components.as_ptr(),
                components.len() as i64,
                out,
                slot,
            )
        })?;
        let linked = component(&mut diagnostics, |out, slot| unsafe {
            (composite.vtable().link)(composite.raw(), out, slot)
        })?;
        let layout = unsafe { (linked.vtable().get_layout)(linked.raw(), 0, diagnostics.slot()) };
        if layout.is_null() {
            return Err(Error::Compile {
                diagnostics: diagnostics.take(),
            });
        }
        Ok(Linked {
            layout: Layout(layout),
            component: linked,
            _composite: composite,
            count: entries.len(),
            _entries: entries,
        })
    }
}

impl Worker {
    fn load(&mut self, module: &str, source: &str) -> Result<*mut c_void> {
        self.loaded += 1;
        let unique = format!("{module}-{}", self.loaded);
        let name = CString::new(unique.as_str()).map_err(|_| Error::Nul("module name"))?;
        let path =
            CString::new(unique + MODULE_EXTENSION).map_err(|_| Error::Nul("module name"))?;
        let text = CString::new(source).map_err(|_| Error::Nul("shader source"))?;
        let mut diagnostics = Diagnostics::default();
        let module = unsafe {
            (self.session.vtable().load_module_from_source_string)(
                self.session.raw(),
                name.as_ptr(),
                path.as_ptr(),
                text.as_ptr(),
                diagnostics.slot(),
            )
        };
        match module.is_null() {
            true => Err(Error::Compile {
                diagnostics: diagnostics.take(),
            }),
            false => Ok(module),
        }
    }
}

struct Linked {
    layout: Layout,
    component: Com<ComponentTypeVtbl>,
    _composite: Com<ComponentTypeVtbl>,
    count: usize,
    _entries: Vec<Com<ComponentTypeVtbl>>,
}

impl Linked {
    fn entry_point(&self, index: usize) -> Result<EntryPoint> {
        let mut diagnostics = Diagnostics::default();
        let mut code = core::ptr::null_mut();
        let result = unsafe {
            (self.component.vtable().get_entry_point_code)(
                self.component.raw(),
                index as i64,
                0,
                &mut code,
                diagnostics.slot(),
            )
        };
        let code = unsafe { Blob::from_owned(code) };
        let (true, Some(code)) = (result >= 0, code) else {
            return Err(Error::Compile {
                diagnostics: diagnostics.take(),
            });
        };
        let words = rikka_spirv::words(code.bytes()).map_err(|error| Error::Compile {
            diagnostics: format!("libslang returned malformed SPIR-V: {error}"),
        })?;
        reflect::assemble(&self.layout, index as u64, words).map(spirv::invariant_entry)
    }

    fn reflection(&self, source: &Source<'_>) -> Result<Reflection> {
        let types = source
            .types
            .iter()
            .filter_map(|name| {
                self.layout
                    .type_layout(name)
                    .transpose()
                    .map(|layout| Ok(((*name).to_owned(), layout?)))
            })
            .collect::<Result<_>>()?;
        let conformances = source
            .conformances
            .iter()
            .map(|&(ty, interface)| {
                Ok((
                    ty.to_owned(),
                    interface.to_owned(),
                    self.layout.conforms(ty, interface)?,
                ))
            })
            .collect::<Result<_>>()?;
        Ok(Reflection {
            parameters: self.layout.parameters()?,
            types,
            conformances,
        })
    }
}

fn global_session() -> Result<Com<GlobalSessionVtbl>> {
    let api = ffi::load().map_err(Error::Unavailable)?;
    let desc = GlobalSessionDesc {
        structure_size: size_of::<GlobalSessionDesc>() as u32,
        api_version: ffi::API_VERSION,
        min_language_version: ffi::LANGUAGE_VERSION_2025,
        enable_glsl: false,
        reserved: [0; 16],
    };
    let mut raw = core::ptr::null_mut();
    let code = unsafe { (api.slang_createGlobalSession2)(&desc, &mut raw) };
    if code < 0 {
        return Err(Error::Unavailable(format!(
            "slang_createGlobalSession2 failed with {code:#x}"
        )));
    }
    unsafe { Com::from_owned(raw) }.ok_or_else(|| {
        Error::Unavailable("slang_createGlobalSession2 returned no session".to_owned())
    })
}

fn open_session(
    global: &Com<GlobalSessionVtbl>,
    settings: &Arc<Settings>,
) -> Result<Com<SessionVtbl>> {
    let profile_name =
        CString::new(rikka_spirv::SLANG_PROFILE).map_err(|_| Error::Nul("profile"))?;
    let profile = unsafe { (global.vtable().find_profile)(global.raw(), profile_name.as_ptr()) };
    let strict =
        CString::new(rikka_spirv::SLANG_WARNINGS_AS_ERRORS).map_err(|_| Error::Nul("warnings"))?;
    let quiet =
        CString::new(rikka_spirv::SLANG_QUIET_WARNINGS).map_err(|_| Error::Nul("warnings"))?;
    let options = [
        int_option(ffi::OPTION_MATRIX_LAYOUT_COLUMN, 1),
        int_option(ffi::OPTION_OPTIMIZATION, settings.optimization.level()),
        string_option(ffi::OPTION_WARNINGS_AS_ERRORS, &strict),
        string_option(ffi::OPTION_DISABLE_WARNINGS, &quiet),
    ];
    let target = TargetDesc {
        structure_size: size_of::<TargetDesc>(),
        format: ffi::TARGET_SPIRV,
        profile,
        flags: ffi::TARGET_FLAG_GENERATE_SPIRV_DIRECTLY,
        floating_point_mode: ffi::FLOATING_POINT_PRECISE,
        line_directive_mode: 0,
        force_glsl_scalar_buffer_layout: true,
        compiler_option_entries: core::ptr::null(),
        compiler_option_entry_count: 0,
    };
    let file_system = Library::into_raw(Arc::clone(settings));
    let search_path = LIBRARY_ROOT.as_ptr();
    let desc = SessionDesc {
        structure_size: size_of::<SessionDesc>(),
        targets: &target,
        target_count: 1,
        flags: 0,
        default_matrix_layout_mode: ffi::MATRIX_LAYOUT_COLUMN_MAJOR,
        search_paths: &search_path,
        search_path_count: 1,
        preprocessor_macros: core::ptr::null(),
        preprocessor_macro_count: 0,
        file_system,
        enable_effect_annotations: false,
        allow_glsl_syntax: false,
        compiler_option_entries: options.as_ptr(),
        compiler_option_entry_count: options.len() as u32,
        skip_spirv_validation: false,
    };
    let mut raw = core::ptr::null_mut();
    let code = unsafe { (global.vtable().create_session)(global.raw(), &desc, &mut raw) };
    unsafe { Library::release(file_system) };
    check("IGlobalSession::createSession", code)?;
    unsafe { Com::from_owned(raw) }.ok_or(Error::Call {
        call: "IGlobalSession::createSession",
        code,
    })
}

fn int_option(name: core::ffi::c_int, value: i32) -> CompilerOptionEntry {
    CompilerOptionEntry {
        name,
        value: CompilerOptionValue {
            kind: ffi::OPTION_KIND_INT,
            int_value0: value,
            int_value1: 0,
            string_value0: core::ptr::null(),
            string_value1: core::ptr::null(),
        },
    }
}

fn string_option(name: core::ffi::c_int, value: &CString) -> CompilerOptionEntry {
    CompilerOptionEntry {
        name,
        value: CompilerOptionValue {
            kind: ffi::OPTION_KIND_STRING,
            int_value0: 0,
            int_value1: 0,
            string_value0: value.as_ptr(),
            string_value1: core::ptr::null(),
        },
    }
}

fn entry_points(module: *mut c_void, wanted: &[&str]) -> Result<Vec<Com<ComponentTypeVtbl>>> {
    let vtable = unsafe { &**module.cast::<*const ModuleVtbl>() };
    let mut found = Vec::new();
    if wanted.is_empty() {
        let count = unsafe { (vtable.get_defined_entry_point_count)(module) };
        for index in 0..count {
            let mut raw = core::ptr::null_mut();
            let code = unsafe { (vtable.get_defined_entry_point)(module, index, &mut raw) };
            check("IModule::getDefinedEntryPoint", code)?;
            found.extend(unsafe { Com::from_owned(raw) });
        }
        return Ok(found);
    }
    for name in wanted {
        let entry = CString::new(*name).map_err(|_| Error::Nul("entry point name"))?;
        let mut raw = core::ptr::null_mut();
        let code = unsafe { (vtable.find_entry_point_by_name)(module, entry.as_ptr(), &mut raw) };
        match (code >= 0, unsafe { Com::from_owned(raw) }) {
            (true, Some(point)) => found.push(point),
            _ => return Err(Error::MissingEntryPoint((*name).to_owned())),
        }
    }
    Ok(found)
}

fn component(
    diagnostics: &mut Diagnostics,
    call: impl FnOnce(*mut *mut c_void, *mut *mut c_void) -> ffi::SlangResult,
) -> Result<Com<ComponentTypeVtbl>> {
    let mut raw = core::ptr::null_mut();
    let code = call(&mut raw, diagnostics.slot());
    match (code >= 0, unsafe { Com::from_owned(raw) }) {
        (true, Some(component)) => Ok(component),
        _ => Err(Error::Compile {
            diagnostics: diagnostics.take(),
        }),
    }
}

use core::ffi::{c_char, c_int, c_uint, c_void};
use std::sync::OnceLock;

use libloading::Library;

use super::{
    GlobalSessionDesc, Reflection, ReflectionEntryPoint, ReflectionType, ReflectionTypeLayout,
    ReflectionUserAttribute, ReflectionVariable, ReflectionVariableLayout, SlangResult, SlangUInt,
    spirv_tools,
};
use crate::exit;

#[cfg(unix)]
const SONAMES: [&str; 2] = ["libslang-compiler.so", "libslang.so"];
#[cfg(windows)]
const SONAMES: [&str; 2] = ["slang-compiler.dll", "slang.dll"];

macro_rules! functions {
    ($(fn $name:ident($($arg:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        #[allow(non_snake_case)]
        pub(crate) struct Functions {
            $(pub(crate) $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
        }

        impl Functions {
            fn resolve(library: &'static Library, soname: &str) -> Result<Self, String> {
                Ok(Self {
                    $($name: unsafe {
                        *library
                            .get::<unsafe extern "C" fn($($arg),*) $(-> $ret)?>(
                                concat!(stringify!($name), "\0").as_bytes(),
                            )
                            .map_err(|error| {
                                format!("{soname} has no `{}`: {error}", stringify!($name))
                            })?
                    },)*
                })
            }
        }
    };
}

functions! {
    fn slang_createGlobalSession2(*const GlobalSessionDesc, *mut *mut c_void) -> SlangResult;
    fn slang_createBlob(*const c_void, usize) -> *mut c_void;

    fn spReflection_getEntryPointByIndex(*mut Reflection, SlangUInt) -> *mut ReflectionEntryPoint;
    fn spReflection_GetParameterCount(*mut Reflection) -> c_uint;
    fn spReflection_GetParameterByIndex(*mut Reflection, c_uint) -> *mut ReflectionVariableLayout;
    fn spReflection_FindTypeByName(*mut Reflection, *const c_char) -> *mut ReflectionType;
    fn spReflection_GetTypeLayout(*mut Reflection, *mut ReflectionType, c_uint) -> *mut ReflectionTypeLayout;
    fn spReflection_isSubType(*mut Reflection, *mut ReflectionType, *mut ReflectionType) -> bool;

    fn spReflectionEntryPoint_getName(*mut ReflectionEntryPoint) -> *const c_char;
    fn spReflectionEntryPoint_getStage(*mut ReflectionEntryPoint) -> u32;
    fn spReflectionEntryPoint_getComputeThreadGroupSize(*mut ReflectionEntryPoint, SlangUInt, *mut SlangUInt);

    fn spReflectionVariableLayout_GetVariable(*mut ReflectionVariableLayout) -> *mut ReflectionVariable;
    fn spReflectionVariableLayout_GetTypeLayout(*mut ReflectionVariableLayout) -> *mut ReflectionTypeLayout;
    fn spReflectionVariableLayout_GetOffset(*mut ReflectionVariableLayout, c_uint) -> usize;
    fn spReflectionVariableLayout_GetSpace(*mut ReflectionVariableLayout, c_uint) -> usize;

    fn spReflectionVariable_GetName(*mut ReflectionVariable) -> *const c_char;
    fn spReflectionVariable_GetUserAttributeCount(*mut ReflectionVariable) -> c_uint;
    fn spReflectionVariable_GetUserAttribute(*mut ReflectionVariable, c_uint) -> *mut ReflectionUserAttribute;

    fn spReflectionTypeLayout_GetType(*mut ReflectionTypeLayout) -> *mut ReflectionType;
    fn spReflectionTypeLayout_getKind(*mut ReflectionTypeLayout) -> u32;
    fn spReflectionTypeLayout_GetSize(*mut ReflectionTypeLayout, c_uint) -> usize;
    fn spReflectionTypeLayout_GetFieldCount(*mut ReflectionTypeLayout) -> u32;
    fn spReflectionTypeLayout_GetFieldByIndex(*mut ReflectionTypeLayout, c_uint) -> *mut ReflectionVariableLayout;
    fn spReflectionTypeLayout_GetElementStride(*mut ReflectionTypeLayout, c_uint) -> usize;
    fn spReflectionTypeLayout_GetElementTypeLayout(*mut ReflectionTypeLayout) -> *mut ReflectionTypeLayout;
    fn spReflectionTypeLayout_GetParameterCategory(*mut ReflectionTypeLayout) -> c_uint;

    fn spReflectionType_GetName(*mut ReflectionType) -> *const c_char;
    fn spReflectionType_GetElementCount(*mut ReflectionType) -> usize;
    fn spReflectionType_GetRowCount(*mut ReflectionType) -> c_uint;
    fn spReflectionType_GetColumnCount(*mut ReflectionType) -> c_uint;
    fn spReflectionType_GetScalarType(*mut ReflectionType) -> u32;
    fn spReflectionType_GetResourceShape(*mut ReflectionType) -> u32;
    fn spReflectionType_GetResourceAccess(*mut ReflectionType) -> u32;

    fn spReflectionUserAttribute_GetName(*mut ReflectionUserAttribute) -> *const c_char;
    fn spReflectionUserAttribute_GetArgumentCount(*mut ReflectionUserAttribute) -> c_uint;
    fn spReflectionUserAttribute_GetArgumentType(*mut ReflectionUserAttribute, c_uint) -> *mut ReflectionType;
    fn spReflectionUserAttribute_GetArgumentValueInt(*mut ReflectionUserAttribute, c_uint, *mut c_int) -> SlangResult;
    fn spReflectionUserAttribute_GetArgumentValueFloat(*mut ReflectionUserAttribute, c_uint, *mut f32) -> SlangResult;
    fn spReflectionUserAttribute_GetArgumentValueString(*mut ReflectionUserAttribute, c_uint, *mut usize) -> *const c_char;
}

static FUNCTIONS: OnceLock<Result<Functions, String>> = OnceLock::new();

static GUARDED: OnceLock<Result<(), String>> = OnceLock::new();

pub(crate) fn load() -> Result<&'static Functions, String> {
    let functions = FUNCTIONS.get_or_init(open).as_ref().map_err(Clone::clone)?;
    GUARDED
        .get_or_init(|| {
            spirv_tools::preload();
            exit::hold()
        })
        .clone()?;
    Ok(functions)
}

fn open() -> Result<Functions, String> {
    let mut failures = Vec::new();
    for soname in SONAMES {
        match open_library(soname) {
            Ok(library) => {
                let library: &'static Library = Box::leak(Box::new(library));
                return Functions::resolve(library, soname);
            }
            Err(error) => failures.push(format!("{soname}: {error}")),
        }
    }
    Err(failures.join("; "))
}

#[cfg(unix)]
fn open_library(soname: &str) -> Result<Library, libloading::Error> {
    unsafe { Library::new(soname) }
}

#[cfg(windows)]
fn open_library(name: &str) -> Result<Library, libloading::Error> {
    load_beside(name).map(Library::from)
}

#[cfg(windows)]
pub(crate) fn load_beside(
    name: &str,
) -> Result<libloading::os::windows::Library, libloading::Error> {
    use libloading::os::windows::{
        LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
        Library as WindowsLibrary,
    };
    unsafe {
        WindowsLibrary::load_with_flags(
            name,
            LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    }
}

pub(crate) fn functions() -> &'static Functions {
    match FUNCTIONS.get() {
        Some(Ok(functions)) => functions,
        _ => unreachable!("libslang is loaded before any Slang object exists"),
    }
}

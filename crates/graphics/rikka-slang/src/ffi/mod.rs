mod library;
mod spirv_tools;

use core::ffi::{c_char, c_int, c_uint, c_void};

#[cfg(windows)]
pub(crate) use library::load_beside;
pub(crate) use library::{functions, load};
pub(crate) use spirv_tools::{SPV_ENV_UNIVERSAL_1_6, SPV_SUCCESS, SpvBinary, spirv_tools};

pub(crate) type SlangResult = i32;
pub(crate) type SlangInt = i64;
pub(crate) type SlangUInt = u64;

pub(crate) const E_NO_INTERFACE: SlangResult = 0x8000_4002_u32 as i32;
pub(crate) const E_NOT_FOUND: SlangResult = 0x8200_0005_u32 as i32;

pub(crate) const API_VERSION: u32 = 0;
pub(crate) const LANGUAGE_VERSION_2025: u32 = 2025;

pub(crate) const TARGET_SPIRV: c_int = 6;
pub(crate) const TARGET_FLAG_GENERATE_SPIRV_DIRECTLY: c_uint = 1 << 10;
pub(crate) const MATRIX_LAYOUT_COLUMN_MAJOR: c_uint = 2;

pub(crate) const STAGE_VERTEX: u32 = 1;
pub(crate) const STAGE_FRAGMENT: u32 = 5;
pub(crate) const STAGE_COMPUTE: u32 = 6;

pub(crate) const OPTION_WARNINGS_AS_ERRORS: c_int = 19;
pub(crate) const OPTION_DISABLE_WARNINGS: c_int = 20;
pub(crate) const OPTION_OPTIMIZATION: c_int = 46;
pub(crate) const OPTION_MATRIX_LAYOUT_COLUMN: c_int = 8;

pub(crate) const OPTION_KIND_INT: c_int = 0;
pub(crate) const OPTION_KIND_STRING: c_int = 1;

pub(crate) const OPTIMIZATION_HIGH: i32 = 2;
pub(crate) const OPTIMIZATION_NONE: i32 = 0;

pub(crate) const FLOATING_POINT_PRECISE: c_uint = 2;

pub(crate) const TYPE_KIND_STRUCT: u32 = 1;
pub(crate) const TYPE_KIND_ARRAY: u32 = 2;
pub(crate) const TYPE_KIND_MATRIX: u32 = 3;
pub(crate) const TYPE_KIND_VECTOR: u32 = 4;
pub(crate) const TYPE_KIND_SCALAR: u32 = 5;
pub(crate) const TYPE_KIND_CONSTANT_BUFFER: u32 = 6;
pub(crate) const TYPE_KIND_RESOURCE: u32 = 7;
pub(crate) const TYPE_KIND_SAMPLER_STATE: u32 = 8;
pub(crate) const TYPE_KIND_PARAMETER_BLOCK: u32 = 11;
pub(crate) const TYPE_KIND_POINTER: u32 = 18;

pub(crate) const SCALAR_BOOL: u32 = 2;
pub(crate) const SCALAR_INT32: u32 = 3;
pub(crate) const SCALAR_UINT32: u32 = 4;
pub(crate) const SCALAR_INT64: u32 = 5;
pub(crate) const SCALAR_UINT64: u32 = 6;
pub(crate) const SCALAR_FLOAT16: u32 = 7;
pub(crate) const SCALAR_FLOAT32: u32 = 8;
pub(crate) const SCALAR_FLOAT64: u32 = 9;
pub(crate) const SCALAR_INT8: u32 = 10;
pub(crate) const SCALAR_UINT8: u32 = 11;
pub(crate) const SCALAR_INT16: u32 = 12;
pub(crate) const SCALAR_UINT16: u32 = 13;

pub(crate) const RESOURCE_BASE_SHAPE_MASK: u32 = 0x0F;
pub(crate) const RESOURCE_TEXTURE_1D: u32 = 0x01;
pub(crate) const RESOURCE_TEXTURE_2D: u32 = 0x02;
pub(crate) const RESOURCE_TEXTURE_3D: u32 = 0x03;
pub(crate) const RESOURCE_TEXTURE_CUBE: u32 = 0x04;
pub(crate) const RESOURCE_TEXTURE_BUFFER: u32 = 0x05;
pub(crate) const RESOURCE_STRUCTURED_BUFFER: u32 = 0x06;
pub(crate) const RESOURCE_BYTE_ADDRESS_BUFFER: u32 = 0x07;
pub(crate) const RESOURCE_TEXTURE_ARRAY_FLAG: u32 = 0x40;

pub(crate) const RESOURCE_ACCESS_READ: u32 = 1;

pub(crate) const CATEGORY_UNIFORM: c_uint = 8;
pub(crate) const CATEGORY_DESCRIPTOR_TABLE_SLOT: c_uint = 9;
pub(crate) const CATEGORY_PUSH_CONSTANT_BUFFER: c_uint = 11;

pub(crate) const LAYOUT_RULES_DEFAULT: c_uint = 0;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Uuid {
    pub(crate) data1: u32,
    pub(crate) data2: u16,
    pub(crate) data3: u16,
    pub(crate) data4: [u8; 8],
}

pub(crate) const UUID_UNKNOWN: Uuid = Uuid {
    data1: 0x0000_0000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

pub(crate) const UUID_CASTABLE: Uuid = Uuid {
    data1: 0x87ed_e0e1,
    data2: 0x4852,
    data3: 0x44b0,
    data4: [0x8b, 0xf2, 0xcb, 0x31, 0x87, 0x4d, 0xe2, 0x39],
};

pub(crate) const UUID_FILE_SYSTEM: Uuid = Uuid {
    data1: 0x003A_09FC,
    data2: 0x3A4D,
    data3: 0x4BA0,
    data4: [0xAD, 0x60, 0x1F, 0xD8, 0x63, 0xA9, 0x15, 0xAB],
};

#[repr(C)]
pub(crate) struct GlobalSessionDesc {
    pub(crate) structure_size: u32,
    pub(crate) api_version: u32,
    pub(crate) min_language_version: u32,
    pub(crate) enable_glsl: bool,
    pub(crate) reserved: [u32; 16],
}

#[repr(C)]
pub(crate) struct CompilerOptionValue {
    pub(crate) kind: c_int,
    pub(crate) int_value0: i32,
    pub(crate) int_value1: i32,
    pub(crate) string_value0: *const c_char,
    pub(crate) string_value1: *const c_char,
}

#[repr(C)]
pub(crate) struct CompilerOptionEntry {
    pub(crate) name: c_int,
    pub(crate) value: CompilerOptionValue,
}

#[repr(C)]
pub(crate) struct TargetDesc {
    pub(crate) structure_size: usize,
    pub(crate) format: c_int,
    pub(crate) profile: c_uint,
    pub(crate) flags: c_uint,
    pub(crate) floating_point_mode: c_uint,
    pub(crate) line_directive_mode: c_uint,
    pub(crate) force_glsl_scalar_buffer_layout: bool,
    pub(crate) compiler_option_entries: *const CompilerOptionEntry,
    pub(crate) compiler_option_entry_count: u32,
}

#[repr(C)]
pub(crate) struct SessionDesc {
    pub(crate) structure_size: usize,
    pub(crate) targets: *const TargetDesc,
    pub(crate) target_count: SlangInt,
    pub(crate) flags: u32,
    pub(crate) default_matrix_layout_mode: c_uint,
    pub(crate) search_paths: *const *const c_char,
    pub(crate) search_path_count: SlangInt,
    pub(crate) preprocessor_macros: *const c_void,
    pub(crate) preprocessor_macro_count: SlangInt,
    pub(crate) file_system: *mut c_void,
    pub(crate) enable_effect_annotations: bool,
    pub(crate) allow_glsl_syntax: bool,
    pub(crate) compiler_option_entries: *const CompilerOptionEntry,
    pub(crate) compiler_option_entry_count: u32,
    pub(crate) skip_spirv_validation: bool,
}

pub(crate) type QueryInterface =
    unsafe extern "C" fn(*mut c_void, *const Uuid, *mut *mut c_void) -> SlangResult;
pub(crate) type AddRef = unsafe extern "C" fn(*mut c_void) -> u32;
pub(crate) type Release = unsafe extern "C" fn(*mut c_void) -> u32;

#[repr(C)]
pub(crate) struct UnknownVtbl {
    pub(crate) query_interface: QueryInterface,
    pub(crate) add_ref: AddRef,
    pub(crate) release: Release,
}

#[repr(C)]
pub(crate) struct BlobVtbl {
    pub(crate) unknown: UnknownVtbl,
    pub(crate) get_buffer_pointer: unsafe extern "C" fn(*mut c_void) -> *const c_void,
    pub(crate) get_buffer_size: unsafe extern "C" fn(*mut c_void) -> usize,
}

#[repr(C)]
pub(crate) struct FileSystemVtbl {
    pub(crate) unknown: UnknownVtbl,
    pub(crate) cast_as: unsafe extern "C" fn(*mut c_void, *const Uuid) -> *mut c_void,
    pub(crate) load_file:
        unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> SlangResult,
}

#[repr(C)]
pub(crate) struct GlobalSessionVtbl {
    pub(crate) unknown: UnknownVtbl,
    pub(crate) create_session:
        unsafe extern "C" fn(*mut c_void, *const SessionDesc, *mut *mut c_void) -> SlangResult,
    pub(crate) find_profile: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_uint,
}

type Diagnostics = *mut *mut c_void;

#[repr(C)]
pub(crate) struct SessionVtbl {
    pub(crate) unknown: UnknownVtbl,
    pub(crate) get_global_session: usize,
    pub(crate) load_module: usize,
    pub(crate) load_module_from_source: usize,
    pub(crate) create_composite_component_type: unsafe extern "C" fn(
        *mut c_void,
        *const *mut c_void,
        SlangInt,
        *mut *mut c_void,
        Diagnostics,
    ) -> SlangResult,
    pub(crate) specialize_type: usize,
    pub(crate) get_type_layout: usize,
    pub(crate) get_container_type: usize,
    pub(crate) get_dynamic_type: usize,
    pub(crate) get_type_rtti_mangled_name: usize,
    pub(crate) get_type_conformance_witness_mangled_name: usize,
    pub(crate) get_type_conformance_witness_sequential_id: usize,
    pub(crate) create_compile_request: usize,
    pub(crate) create_type_conformance_component_type: usize,
    pub(crate) load_module_from_ir_blob: usize,
    pub(crate) get_loaded_module_count: usize,
    pub(crate) get_loaded_module: usize,
    pub(crate) is_binary_module_up_to_date: usize,
    pub(crate) load_module_from_source_string: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *const c_char,
        *const c_char,
        Diagnostics,
    ) -> *mut c_void,
}

#[repr(C)]
pub(crate) struct ComponentTypeVtbl {
    pub(crate) unknown: UnknownVtbl,
    pub(crate) get_session: usize,
    pub(crate) get_layout: unsafe extern "C" fn(*mut c_void, SlangInt, Diagnostics) -> *mut c_void,
    pub(crate) get_specialization_param_count: usize,
    pub(crate) get_entry_point_code: unsafe extern "C" fn(
        *mut c_void,
        SlangInt,
        SlangInt,
        *mut *mut c_void,
        Diagnostics,
    ) -> SlangResult,
    pub(crate) get_result_as_file_system: usize,
    pub(crate) get_entry_point_hash: usize,
    pub(crate) specialize: usize,
    pub(crate) link:
        unsafe extern "C" fn(*mut c_void, *mut *mut c_void, Diagnostics) -> SlangResult,
    pub(crate) get_entry_point_host_callable: usize,
    pub(crate) rename_entry_point: usize,
    pub(crate) link_with_options: usize,
    pub(crate) get_target_code: usize,
    pub(crate) get_target_metadata: usize,
    pub(crate) get_entry_point_metadata: usize,
}

#[repr(C)]
pub(crate) struct ModuleVtbl {
    pub(crate) component: ComponentTypeVtbl,
    pub(crate) find_entry_point_by_name:
        unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> SlangResult,
    pub(crate) get_defined_entry_point_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub(crate) get_defined_entry_point:
        unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> SlangResult,
}

pub(crate) type Reflection = c_void;
pub(crate) type ReflectionType = c_void;
pub(crate) type ReflectionTypeLayout = c_void;
pub(crate) type ReflectionVariable = c_void;
pub(crate) type ReflectionVariableLayout = c_void;
pub(crate) type ReflectionEntryPoint = c_void;
pub(crate) type ReflectionUserAttribute = c_void;

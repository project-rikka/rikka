use core::ffi::{CStr, c_char, c_int};

use crate::error::{Error, Result};
use crate::ffi::{
    self, Reflection, ReflectionType, ReflectionTypeLayout, ReflectionVariableLayout, functions,
};
use crate::layout::{
    Access, Argument, Attribute, Binding, EntryPoint, Field, Parameter, Scalar, Shape, Stage, Type,
    TypeLayout,
};

const UNSIZED: usize = usize::MAX;

fn text(raw: *const c_char) -> String {
    if raw.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

pub(crate) struct Layout(pub(crate) *mut Reflection);

impl Layout {
    pub(crate) fn entry_point(&self, index: u64) -> Result<(String, Stage, Option<[u32; 3]>)> {
        let api = functions();
        unsafe {
            let entry = (api.spReflection_getEntryPointByIndex)(self.0, index);
            let name = text((api.spReflectionEntryPoint_getName)(entry));
            let stage = match (api.spReflectionEntryPoint_getStage)(entry) {
                ffi::STAGE_VERTEX => Stage::Vertex,
                ffi::STAGE_FRAGMENT => Stage::Fragment,
                ffi::STAGE_COMPUTE => Stage::Compute,
                stage => return Err(Error::UnsupportedStage { name, stage }),
            };
            let workgroup_size = (stage == Stage::Compute).then(|| {
                let mut size = [0u64; 3];
                (api.spReflectionEntryPoint_getComputeThreadGroupSize)(entry, 3, size.as_mut_ptr());
                size.map(|axis| axis as u32)
            });
            Ok((name, stage, workgroup_size))
        }
    }

    pub(crate) fn parameters(&self) -> Result<Vec<Parameter>> {
        let api = functions();
        unsafe {
            (0..(api.spReflection_GetParameterCount)(self.0))
                .map(|index| parameter((api.spReflection_GetParameterByIndex)(self.0, index)))
                .collect()
        }
    }

    pub(crate) fn conforms(&self, ty: &str, interface: &str) -> Result<bool> {
        let api = functions();
        let find = |name: &str| -> Result<*mut ReflectionType> {
            let wanted = std::ffi::CString::new(name).map_err(|_| Error::Nul("type name"))?;
            let found = unsafe { (api.spReflection_FindTypeByName)(self.0, wanted.as_ptr()) };
            match found.is_null() {
                true => Err(Error::UnknownType(name.to_owned())),
                false => Ok(found),
            }
        };
        let (sub, interface) = (find(ty)?, find(interface)?);
        Ok(unsafe { (api.spReflection_isSubType)(self.0, sub, interface) })
    }

    pub(crate) fn type_layout(&self, name: &str) -> Result<Option<TypeLayout>> {
        let api = functions();
        let wanted = std::ffi::CString::new(name).map_err(|_| Error::Nul("type name"))?;
        unsafe {
            let ty = (api.spReflection_FindTypeByName)(self.0, wanted.as_ptr());
            if ty.is_null() {
                return Ok(None);
            }
            let layout = (api.spReflection_GetTypeLayout)(self.0, ty, ffi::LAYOUT_RULES_DEFAULT);
            if layout.is_null() {
                return Ok(None);
            }
            type_layout(layout).map(Some)
        }
    }
}

pub(crate) fn assemble(layout: &Layout, index: u64, spirv: Vec<u32>) -> Result<EntryPoint> {
    let (name, stage, workgroup_size) = layout.entry_point(index)?;
    Ok(EntryPoint {
        name,
        stage,
        workgroup_size,
        spirv,
    })
}

unsafe fn parameter(variable: *mut ReflectionVariableLayout) -> Result<Parameter> {
    let api = functions();
    unsafe {
        let layout = (api.spReflectionVariableLayout_GetTypeLayout)(variable);
        let category = (api.spReflectionTypeLayout_GetParameterCategory)(layout);
        let binding = match category {
            ffi::CATEGORY_DESCRIPTOR_TABLE_SLOT => Binding::Descriptor {
                set: (api.spReflectionVariableLayout_GetSpace)(variable, category) as u32,
                binding: (api.spReflectionVariableLayout_GetOffset)(variable, category) as u32,
            },
            ffi::CATEGORY_PUSH_CONSTANT_BUFFER => Binding::PushConstant,
            ffi::CATEGORY_UNIFORM => Binding::Uniform {
                offset: (api.spReflectionVariableLayout_GetOffset)(variable, category),
            },
            _ => Binding::Unbound,
        };
        Ok(Parameter {
            name: text((api.spReflectionVariable_GetName)((api
                .spReflectionVariableLayout_GetVariable)(
                variable
            ))),
            binding,
            layout: type_layout(layout)?,
        })
    }
}

unsafe fn type_layout(layout: *mut ReflectionTypeLayout) -> Result<TypeLayout> {
    let api = functions();
    unsafe {
        let ty = (api.spReflectionTypeLayout_GetType)(layout);
        let size = (api.spReflectionTypeLayout_GetSize)(layout, ffi::CATEGORY_UNIFORM);
        let described = match (api.spReflectionTypeLayout_getKind)(layout) {
            ffi::TYPE_KIND_SCALAR => scalar((api.spReflectionType_GetScalarType)(ty)).map_or_else(
                || Type::Other(text((api.spReflectionType_GetName)(ty))),
                Type::Scalar,
            ),
            ffi::TYPE_KIND_VECTOR => match scalar((api.spReflectionType_GetScalarType)(ty)) {
                Some(scalar) => Type::Vector {
                    scalar,
                    count: (api.spReflectionType_GetElementCount)(ty) as u32,
                },
                None => Type::Other(text((api.spReflectionType_GetName)(ty))),
            },
            ffi::TYPE_KIND_MATRIX => match scalar((api.spReflectionType_GetScalarType)(ty)) {
                Some(scalar) => Type::Matrix {
                    scalar,
                    rows: (api.spReflectionType_GetRowCount)(ty),
                    columns: (api.spReflectionType_GetColumnCount)(ty),
                },
                None => Type::Other(text((api.spReflectionType_GetName)(ty))),
            },
            ffi::TYPE_KIND_ARRAY => {
                let count = (api.spReflectionType_GetElementCount)(ty);
                Type::Array {
                    element: Box::new(type_layout((api
                        .spReflectionTypeLayout_GetElementTypeLayout)(
                        layout
                    ))?),
                    count: (count != 0 && count != UNSIZED).then_some(count),
                    stride: (api.spReflectionTypeLayout_GetElementStride)(
                        layout,
                        ffi::CATEGORY_UNIFORM,
                    ),
                }
            }
            ffi::TYPE_KIND_STRUCT => Type::Struct {
                name: text((api.spReflectionType_GetName)(ty)),
                fields: (0..(api.spReflectionTypeLayout_GetFieldCount)(layout))
                    .map(|index| field((api.spReflectionTypeLayout_GetFieldByIndex)(layout, index)))
                    .collect::<Result<_>>()?,
            },
            ffi::TYPE_KIND_POINTER => Type::Pointer {
                pointee: text((api.spReflectionType_GetName)((api
                    .spReflectionTypeLayout_GetType)(
                    (api.spReflectionTypeLayout_GetElementTypeLayout)(layout),
                ))),
            },
            ffi::TYPE_KIND_CONSTANT_BUFFER | ffi::TYPE_KIND_PARAMETER_BLOCK => {
                Type::ConstantBuffer(Box::new(type_layout((api
                    .spReflectionTypeLayout_GetElementTypeLayout)(
                    layout
                ))?))
            }
            ffi::TYPE_KIND_RESOURCE => {
                let shape = (api.spReflectionType_GetResourceShape)(ty);
                Type::Resource {
                    shape: match shape & ffi::RESOURCE_BASE_SHAPE_MASK {
                        ffi::RESOURCE_TEXTURE_1D => Shape::Texture1d,
                        ffi::RESOURCE_TEXTURE_2D => Shape::Texture2d,
                        ffi::RESOURCE_TEXTURE_3D => Shape::Texture3d,
                        ffi::RESOURCE_TEXTURE_CUBE => Shape::TextureCube,
                        ffi::RESOURCE_TEXTURE_BUFFER => Shape::TextureBuffer,
                        ffi::RESOURCE_STRUCTURED_BUFFER => Shape::StructuredBuffer,
                        ffi::RESOURCE_BYTE_ADDRESS_BUFFER => Shape::ByteAddressBuffer,
                        _ => Shape::Other,
                    },
                    arrayed: shape & ffi::RESOURCE_TEXTURE_ARRAY_FLAG != 0,
                    access: match (api.spReflectionType_GetResourceAccess)(ty) {
                        ffi::RESOURCE_ACCESS_READ => Access::Read,
                        _ => Access::ReadWrite,
                    },
                }
            }
            ffi::TYPE_KIND_SAMPLER_STATE => Type::Sampler,
            _ => Type::Other(text((api.spReflectionType_GetName)(ty))),
        };
        Ok(TypeLayout {
            ty: described,
            size,
        })
    }
}

unsafe fn field(variable: *mut ReflectionVariableLayout) -> Result<Field> {
    let api = functions();
    unsafe {
        let declared = (api.spReflectionVariableLayout_GetVariable)(variable);
        Ok(Field {
            name: text((api.spReflectionVariable_GetName)(declared)),
            offset: (api.spReflectionVariableLayout_GetOffset)(variable, ffi::CATEGORY_UNIFORM),
            layout: type_layout((api.spReflectionVariableLayout_GetTypeLayout)(variable))?,
            attributes: (0..(api.spReflectionVariable_GetUserAttributeCount)(declared))
                .map(|index| {
                    attribute((api.spReflectionVariable_GetUserAttribute)(declared, index))
                })
                .collect::<Result<_>>()?,
        })
    }
}

unsafe fn attribute(raw: *mut ffi::ReflectionUserAttribute) -> Result<Attribute> {
    let api = functions();
    unsafe {
        let arguments = (0..(api.spReflectionUserAttribute_GetArgumentCount)(raw))
            .map(|index| argument(raw, index))
            .collect::<Result<_>>()?;
        Ok(Attribute {
            name: text((api.spReflectionUserAttribute_GetName)(raw)),
            arguments,
        })
    }
}

unsafe fn argument(raw: *mut ffi::ReflectionUserAttribute, index: u32) -> Result<Argument> {
    let api = functions();
    unsafe {
        let mut length = 0usize;
        let string =
            (api.spReflectionUserAttribute_GetArgumentValueString)(raw, index, &mut length);
        if !string.is_null() {
            let bytes = core::slice::from_raw_parts(string.cast::<u8>(), length);
            return Ok(Argument::String(
                String::from_utf8_lossy(bytes).trim_matches('"').to_owned(),
            ));
        }
        let ty = (api.spReflectionUserAttribute_GetArgumentType)(raw, index);
        let floating = !ty.is_null()
            && matches!(
                scalar((api.spReflectionType_GetScalarType)(ty)),
                Some(Scalar::F16 | Scalar::F32 | Scalar::F64)
            );
        if floating {
            let mut value = 0.0f32;
            let code =
                (api.spReflectionUserAttribute_GetArgumentValueFloat)(raw, index, &mut value);
            check("spReflectionUserAttribute_GetArgumentValueFloat", code)?;
            Ok(Argument::Float(value))
        } else {
            let mut value: c_int = 0;
            let code = (api.spReflectionUserAttribute_GetArgumentValueInt)(raw, index, &mut value);
            check("spReflectionUserAttribute_GetArgumentValueInt", code)?;
            Ok(Argument::Int(value))
        }
    }
}

pub(crate) fn check(call: &'static str, code: i32) -> Result<()> {
    if code < 0 {
        return Err(Error::Call { call, code });
    }
    Ok(())
}

fn scalar(raw: u32) -> Option<Scalar> {
    Some(match raw {
        ffi::SCALAR_BOOL => Scalar::Bool,
        ffi::SCALAR_INT8 => Scalar::I8,
        ffi::SCALAR_UINT8 => Scalar::U8,
        ffi::SCALAR_INT16 => Scalar::I16,
        ffi::SCALAR_UINT16 => Scalar::U16,
        ffi::SCALAR_INT32 => Scalar::I32,
        ffi::SCALAR_UINT32 => Scalar::U32,
        ffi::SCALAR_INT64 => Scalar::I64,
        ffi::SCALAR_UINT64 => Scalar::U64,
        ffi::SCALAR_FLOAT16 => Scalar::F16,
        ffi::SCALAR_FLOAT32 => Scalar::F32,
        ffi::SCALAR_FLOAT64 => Scalar::F64,
        _ => return None,
    })
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryPoint {
    pub name: String,
    pub stage: Stage,
    pub workgroup_size: Option<[u32; 3]>,
    pub spirv: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scalar {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F16,
    F32,
    F64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Texture1d,
    Texture2d,
    Texture3d,
    TextureCube,
    TextureBuffer,
    StructuredBuffer,
    ByteAddressBuffer,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Read,
    ReadWrite,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeLayout {
    pub ty: Type,
    pub size: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    Scalar(Scalar),
    Vector {
        scalar: Scalar,
        count: u32,
    },
    Matrix {
        scalar: Scalar,
        rows: u32,
        columns: u32,
    },
    Array {
        element: Box<TypeLayout>,
        count: Option<usize>,
        stride: usize,
    },
    Struct {
        name: String,
        fields: Vec<Field>,
    },
    Pointer {
        pointee: String,
    },
    ConstantBuffer(Box<TypeLayout>),
    Resource {
        shape: Shape,
        arrayed: bool,
        access: Access,
    },
    Sampler,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub offset: usize,
    pub layout: TypeLayout,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,
    pub arguments: Vec<Argument>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Argument {
    Int(i32),
    Float(f32),
    String(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Binding {
    Descriptor { set: u32, binding: u32 },
    PushConstant,
    Uniform { offset: usize },
    Unbound,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub binding: Binding,
    pub layout: TypeLayout,
}

impl Type {
    pub fn fields(&self) -> &[Field] {
        match self {
            Self::Struct { fields, .. } => fields,
            _ => &[],
        }
    }
}

impl Field {
    pub fn attribute(&self, name: &str) -> Option<&Attribute> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name == name)
    }
}

impl Argument {
    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            Self::Float(value) => Some(value),
            Self::Int(value) => Some(value as f32),
            Self::String(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }
}

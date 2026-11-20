use std::collections::{BTreeMap, HashMap};

use crate::error::{Error, Result};
use crate::instruction::{instructions, literal_string};

const OP_CAPABILITY: u32 = 17;
const OP_ENTRY_POINT: u32 = 15;
const OP_TYPE_BOOL: u32 = 20;
const OP_TYPE_INT: u32 = 21;
const OP_TYPE_FLOAT: u32 = 22;
const OP_TYPE_VECTOR: u32 = 23;
const OP_TYPE_MATRIX: u32 = 24;
const OP_TYPE_IMAGE: u32 = 25;
const OP_TYPE_SAMPLER: u32 = 26;
const OP_TYPE_SAMPLED_IMAGE: u32 = 27;
const OP_TYPE_ARRAY: u32 = 28;
const OP_TYPE_RUNTIME_ARRAY: u32 = 29;
const OP_TYPE_STRUCT: u32 = 30;
const OP_TYPE_POINTER: u32 = 32;
const OP_CONSTANT: u32 = 43;
const OP_SPEC_CONSTANT: u32 = 50;
const OP_VARIABLE: u32 = 59;
const OP_DECORATE: u32 = 71;
const OP_MEMBER_DECORATE: u32 = 72;
const OP_FUNCTION: u32 = 54;
const OP_EXECUTION_MODE: u32 = 16;

const MODE_LOCAL_SIZE: u32 = 17;

const DECORATION_ARRAY_STRIDE: u32 = 6;
const DECORATION_MATRIX_STRIDE: u32 = 7;
const DECORATION_BUILT_IN: u32 = 11;
const DECORATION_LOCATION: u32 = 30;
const DECORATION_COMPONENT: u32 = 31;
const DECORATION_BINDING: u32 = 33;
const DECORATION_DESCRIPTOR_SET: u32 = 34;
const DECORATION_OFFSET: u32 = 35;

const STORAGE_UNIFORM_CONSTANT: u32 = 0;
const STORAGE_INPUT: u32 = 1;
const STORAGE_UNIFORM: u32 = 2;
const STORAGE_OUTPUT: u32 = 3;
const STORAGE_PUSH_CONSTANT: u32 = 9;
const STORAGE_STORAGE_BUFFER: u32 = 12;
const STORAGE_PHYSICAL_STORAGE_BUFFER: u32 = 5349;

const MODEL_VERTEX: u32 = 0;
const MODEL_FRAGMENT: u32 = 4;
const MODEL_GL_COMPUTE: u32 = 5;

const DIM_2D: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Vertex,
    Fragment,
    Compute,
    Other(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scalar {
    Float(u32),
    Int(u32),
    UInt(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Descriptor {
    SampledImage2d,
    Image2d,
    Sampler,
    StorageImage2d,
    Buffer,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub set: u32,
    pub binding: u32,
    pub descriptor: Descriptor,
}

pub type Slots = BTreeMap<(u32, u32), Scalar>;

#[derive(Clone, Debug, Default)]
pub struct Reflection {
    pub models: Vec<Model>,
    pub capabilities: Vec<u32>,
    pub push_size: u32,
    pub push_blocks: u32,
    pub local_size: Option<[u32; 3]>,
    pub bindings: Vec<Binding>,
    pub inputs: Slots,
    pub outputs: Slots,
}

impl Reflection {
    #[must_use]
    pub fn uses_storage_images(&self) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.descriptor == Descriptor::StorageImage2d)
    }
}

#[derive(Clone, Debug)]
enum Type {
    Scalar(Scalar),
    Bool,
    Vector(u32, u32),
    Matrix(u32, u32),
    Image {
        dim: u32,
        arrayed: bool,
        multisampled: bool,
        sampled: u32,
    },
    Sampler,
    SampledImage(u32),
    Array(u32, u32),
    RuntimeArray(u32),
    Struct(Vec<u32>),
    Pointer(u32, u32),
}

#[derive(Default)]
struct Decorations {
    location: Option<u32>,
    component: Option<u32>,
    binding: Option<u32>,
    set: Option<u32>,
    built_in: bool,
    array_stride: Option<u32>,
}

#[derive(Default)]
struct MemberDecorations {
    offset: Option<u32>,
    matrix_stride: Option<u32>,
    location: Option<u32>,
    built_in: bool,
}

struct EntryPoint {
    model: u32,
    function: u32,
    interface: Vec<u32>,
}

#[derive(Default)]
struct Module {
    version: u32,
    types: HashMap<u32, Type>,
    constants: HashMap<u32, u32>,
    variables: Vec<(u32, u32, u32)>,
    decorations: HashMap<u32, Decorations>,
    members: HashMap<(u32, u32), MemberDecorations>,
    local_sizes: HashMap<u32, [u32; 3]>,
    entry_points: Vec<(String, EntryPoint)>,
    capabilities: Vec<u32>,
}

pub fn reflect(words: &[u32], entry: &str) -> Result<Reflection> {
    let module = parse(words)?;
    let points: Vec<&EntryPoint> = module
        .entry_points
        .iter()
        .filter(|(name, _)| name == entry)
        .map(|(_, point)| point)
        .collect();
    if points.is_empty() {
        return Err(Error::MissingEntryPoint(entry.to_owned()));
    }

    let mut reflection = Reflection {
        models: points.iter().map(|point| model_of(point.model)).collect(),
        capabilities: module.capabilities.clone(),
        local_size: points
            .iter()
            .find_map(|point| module.local_sizes.get(&point.function).copied()),
        ..Reflection::default()
    };
    let lists_every_global = module.version >= 0x0001_0400;
    for &(type_id, id, storage) in &module.variables {
        let listed = points.iter().any(|point| point.interface.contains(&id));
        let used = listed || !lists_every_global;
        let pointee = match module.types.get(&type_id) {
            Some(Type::Pointer(_, pointee)) => *pointee,
            _ => {
                return Err(Error::NonPointerVariable(id));
            }
        };
        let decorations = module.decorations.get(&id);
        match storage {
            STORAGE_INPUT | STORAGE_OUTPUT if listed => {
                if decorations.is_some_and(|d| d.built_in) || module.is_built_in_block(pointee) {
                    continue;
                }
                let into = match storage {
                    STORAGE_INPUT => &mut reflection.inputs,
                    _ => &mut reflection.outputs,
                };
                let location = decorations.and_then(|d| d.location);
                let component = decorations.and_then(|d| d.component).unwrap_or(0);
                module.cover(pointee, location, component, into)?;
            }
            STORAGE_PUSH_CONSTANT if used => {
                reflection.push_blocks += 1;
                reflection.push_size = reflection.push_size.max(module.size_of(pointee)?);
            }
            STORAGE_UNIFORM_CONSTANT | STORAGE_UNIFORM | STORAGE_STORAGE_BUFFER if used => {
                let descriptor = match storage {
                    STORAGE_UNIFORM_CONSTANT => module.descriptor_of(pointee),
                    _ => Descriptor::Buffer,
                };
                reflection.bindings.push(Binding {
                    set: decorations.and_then(|d| d.set).unwrap_or(0),
                    binding: decorations.and_then(|d| d.binding).unwrap_or(0),
                    descriptor,
                });
            }
            _ => {}
        }
    }
    Ok(reflection)
}

fn model_of(model: u32) -> Model {
    match model {
        MODEL_VERTEX => Model::Vertex,
        MODEL_FRAGMENT => Model::Fragment,
        MODEL_GL_COMPUTE => Model::Compute,
        other => Model::Other(other),
    }
}

fn parse(words: &[u32]) -> Result<Module> {
    let instructions = instructions(words)?;
    let mut module = Module {
        version: instructions.version(),
        ..Module::default()
    };
    for instruction in instructions {
        let instruction = instruction?;
        if instruction.opcode == OP_FUNCTION {
            break;
        }
        module.read(instruction.opcode, instruction.operands)?;
    }
    Ok(module)
}

fn operand(operands: &[u32], index: usize) -> Result<u32> {
    operands.get(index).copied().ok_or(Error::MissingOperand)
}

impl Module {
    fn read(&mut self, opcode: u32, operands: &[u32]) -> Result<()> {
        match opcode {
            OP_CAPABILITY => self.capabilities.push(operand(operands, 0)?),
            OP_ENTRY_POINT => {
                let model = operand(operands, 0)?;
                let function = operand(operands, 1)?;
                let (name, used) = literal_string(&operands[2.min(operands.len())..]);
                let interface = operands.get(2 + used..).unwrap_or_default().to_vec();
                self.entry_points.push((
                    name,
                    EntryPoint {
                        model,
                        function,
                        interface,
                    },
                ));
            }
            OP_EXECUTION_MODE => {
                if operand(operands, 1)? == MODE_LOCAL_SIZE {
                    self.local_sizes.insert(
                        operand(operands, 0)?,
                        [
                            operand(operands, 2)?,
                            operand(operands, 3)?,
                            operand(operands, 4)?,
                        ],
                    );
                }
            }
            OP_TYPE_BOOL => {
                self.types.insert(operand(operands, 0)?, Type::Bool);
            }
            OP_TYPE_INT => {
                let width = operand(operands, 1)?;
                let scalar = match operand(operands, 2)? {
                    0 => Scalar::UInt(width),
                    _ => Scalar::Int(width),
                };
                self.types
                    .insert(operand(operands, 0)?, Type::Scalar(scalar));
            }
            OP_TYPE_FLOAT => {
                let scalar = Scalar::Float(operand(operands, 1)?);
                self.types
                    .insert(operand(operands, 0)?, Type::Scalar(scalar));
            }
            OP_TYPE_VECTOR | OP_TYPE_MATRIX => {
                let (element, count) = (operand(operands, 1)?, operand(operands, 2)?);
                let ty = match opcode {
                    OP_TYPE_VECTOR => Type::Vector(element, count),
                    _ => Type::Matrix(element, count),
                };
                self.types.insert(operand(operands, 0)?, ty);
            }
            OP_TYPE_IMAGE => {
                self.types.insert(
                    operand(operands, 0)?,
                    Type::Image {
                        dim: operand(operands, 2)?,
                        arrayed: operand(operands, 4)? != 0,
                        multisampled: operand(operands, 5)? != 0,
                        sampled: operand(operands, 6)?,
                    },
                );
            }
            OP_TYPE_SAMPLER => {
                self.types.insert(operand(operands, 0)?, Type::Sampler);
            }
            OP_TYPE_SAMPLED_IMAGE => {
                self.types.insert(
                    operand(operands, 0)?,
                    Type::SampledImage(operand(operands, 1)?),
                );
            }
            OP_TYPE_ARRAY => {
                self.types.insert(
                    operand(operands, 0)?,
                    Type::Array(operand(operands, 1)?, operand(operands, 2)?),
                );
            }
            OP_TYPE_RUNTIME_ARRAY => {
                self.types.insert(
                    operand(operands, 0)?,
                    Type::RuntimeArray(operand(operands, 1)?),
                );
            }
            OP_TYPE_STRUCT => {
                self.types
                    .insert(operand(operands, 0)?, Type::Struct(operands[1..].to_vec()));
            }
            OP_TYPE_POINTER => {
                self.types.insert(
                    operand(operands, 0)?,
                    Type::Pointer(operand(operands, 1)?, operand(operands, 2)?),
                );
            }
            OP_CONSTANT | OP_SPEC_CONSTANT => {
                self.constants
                    .insert(operand(operands, 1)?, operand(operands, 2)?);
            }
            OP_VARIABLE => {
                self.variables.push((
                    operand(operands, 0)?,
                    operand(operands, 1)?,
                    operand(operands, 2)?,
                ));
            }
            OP_DECORATE => {
                let decorations = self.decorations.entry(operand(operands, 0)?).or_default();
                let value = operands.get(2).copied();
                match operand(operands, 1)? {
                    DECORATION_LOCATION => decorations.location = value,
                    DECORATION_COMPONENT => decorations.component = value,
                    DECORATION_BINDING => decorations.binding = value,
                    DECORATION_DESCRIPTOR_SET => decorations.set = value,
                    DECORATION_ARRAY_STRIDE => decorations.array_stride = value,
                    DECORATION_BUILT_IN => decorations.built_in = true,
                    _ => {}
                }
            }
            OP_MEMBER_DECORATE => {
                let key = (operand(operands, 0)?, operand(operands, 1)?);
                let decorations = self.members.entry(key).or_default();
                let value = operands.get(3).copied();
                match operand(operands, 2)? {
                    DECORATION_OFFSET => decorations.offset = value,
                    DECORATION_MATRIX_STRIDE => decorations.matrix_stride = value,
                    DECORATION_LOCATION => decorations.location = value,
                    DECORATION_BUILT_IN => decorations.built_in = true,
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn ty(&self, id: u32) -> Result<&Type> {
        self.types.get(&id).ok_or(Error::UndeclaredType(id))
    }

    fn length(&self, id: u32) -> Result<u32> {
        self.constants
            .get(&id)
            .copied()
            .ok_or(Error::NonConstantLength(id))
    }

    fn is_built_in_block(&self, id: u32) -> bool {
        let mut id = id;
        while let Some(Type::Array(element, _) | Type::RuntimeArray(element)) = self.types.get(&id)
        {
            id = *element;
        }
        match self.types.get(&id) {
            Some(Type::Struct(members)) => (0..members.len() as u32).any(|member| {
                self.members
                    .get(&(id, member))
                    .is_some_and(|decorations| decorations.built_in)
            }),
            _ => false,
        }
    }

    fn descriptor_of(&self, id: u32) -> Descriptor {
        let mut id = id;
        while let Some(Type::Array(element, _) | Type::RuntimeArray(element)) = self.types.get(&id)
        {
            id = *element;
        }
        let plain_2d = |image: Option<&Type>, expected: u32| {
            matches!(
                image,
                Some(Type::Image { dim: DIM_2D, arrayed: false, multisampled: false, sampled })
                    if *sampled == expected
            )
        };
        match self.types.get(&id) {
            Some(Type::SampledImage(image)) if plain_2d(self.types.get(image), 1) => {
                Descriptor::SampledImage2d
            }
            Some(image @ Type::Image { .. }) if plain_2d(Some(image), 1) => Descriptor::Image2d,
            Some(image @ Type::Image { .. }) if plain_2d(Some(image), 2) => {
                Descriptor::StorageImage2d
            }
            Some(Type::Sampler) => Descriptor::Sampler,
            _ => Descriptor::Other,
        }
    }

    fn size_of(&self, id: u32) -> Result<u32> {
        let size = match self.ty(id)? {
            Type::Scalar(Scalar::Float(width) | Scalar::Int(width) | Scalar::UInt(width)) => {
                width / 8
            }
            Type::Vector(element, count) => self.size_of(*element)? * count,
            Type::Matrix(column, count) => self.size_of(*column)? * count,
            Type::Array(element, length) => {
                let stride = match self.decorations.get(&id).and_then(|d| d.array_stride) {
                    Some(stride) => stride,
                    None => self.size_of(*element)?,
                };
                stride.saturating_mul(self.length(*length)?)
            }
            Type::Struct(members) => {
                let mut end = 0u32;
                for (member, &ty) in members.iter().enumerate() {
                    let decorations = self.members.get(&(id, member as u32));
                    let offset = decorations.and_then(|d| d.offset).unwrap_or(end);
                    let size = match (self.ty(ty)?, decorations.and_then(|d| d.matrix_stride)) {
                        (Type::Matrix(_, columns), Some(stride)) => stride.saturating_mul(*columns),
                        _ => self.size_of(ty)?,
                    };
                    end = end.max(offset.saturating_add(size));
                }
                end
            }
            Type::Pointer(STORAGE_PHYSICAL_STORAGE_BUFFER, _) => 8,
            Type::Bool
            | Type::Image { .. }
            | Type::Sampler
            | Type::SampledImage(_)
            | Type::RuntimeArray(_)
            | Type::Pointer(..) => {
                return Err(Error::UnsizedPushMember(id));
            }
        };
        Ok(size)
    }

    fn locations_of(&self, id: u32) -> Result<u32> {
        Ok(match self.ty(id)? {
            Type::Scalar(_) => 1,
            Type::Vector(element, count) => (self.words_of(*element)? * count).div_ceil(4),
            Type::Matrix(column, count) => self.locations_of(*column)? * count,
            Type::Array(element, length) => self.locations_of(*element)? * self.length(*length)?,
            Type::Struct(members) => {
                let mut total = 0;
                for &member in members {
                    total += self.locations_of(member)?;
                }
                total
            }
            _ => {
                return Err(Error::UnlocatedInterface);
            }
        })
    }

    fn words_of(&self, id: u32) -> Result<u32> {
        match self.ty(id)? {
            Type::Scalar(Scalar::Float(width) | Scalar::Int(width) | Scalar::UInt(width)) => {
                Ok(width.div_ceil(32))
            }
            _ => Err(Error::NonNumericInterface),
        }
    }

    fn scalar_of(&self, id: u32) -> Result<Scalar> {
        match self.ty(id)? {
            Type::Scalar(scalar) => Ok(*scalar),
            _ => Err(Error::NonNumericInterface),
        }
    }

    fn cover(
        &self,
        id: u32,
        location: Option<u32>,
        component: u32,
        into: &mut Slots,
    ) -> Result<()> {
        let fill = |scalar: Scalar, count: u32, words: u32, location: u32, into: &mut Slots| {
            for word in 0..count * words {
                let at = component + word;
                into.insert((location + at / 4, at % 4), scalar);
            }
        };
        if let Type::Struct(members) = self.ty(id)? {
            let mut next = location;
            for (index, &member) in members.iter().enumerate() {
                let decorations = self.members.get(&(id, index as u32));
                if decorations.is_some_and(|d| d.built_in) {
                    continue;
                }
                let at = decorations.and_then(|d| d.location).or(next);
                self.cover(member, at, 0, into)?;
                next = match at {
                    Some(at) => Some(at + self.locations_of(member)?),
                    None => None,
                };
            }
            return Ok(());
        }
        let Some(location) = location else {
            return Err(Error::UnlocatedInterface);
        };
        match self.ty(id)? {
            Type::Scalar(scalar) => fill(*scalar, 1, self.words_of(id)?, location, into),
            Type::Vector(element, count) => {
                fill(
                    self.scalar_of(*element)?,
                    *count,
                    self.words_of(*element)?,
                    location,
                    into,
                );
            }
            Type::Matrix(column, count) => {
                let step = self.locations_of(*column)?;
                for index in 0..*count {
                    self.cover(*column, Some(location + index * step), 0, into)?;
                }
            }
            Type::Array(element, length) => {
                let step = self.locations_of(*element)?;
                for index in 0..self.length(*length)? {
                    self.cover(*element, Some(location + index * step), component, into)?;
                }
            }
            _ => {
                return Err(Error::NonNumericInterface);
            }
        }
        Ok(())
    }
}

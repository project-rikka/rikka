use glam::{Vec2, Vec3, Vec4};

use crate::parallel;
const LUMINANCE: Vec3 = Vec3::new(0.212_672_9, 0.715_152_2, 0.072_175);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DataType {
    Bool,
    Int,
    Float,
    Vec2,
    Vec3,
    Color,
}

impl DataType {
    pub const ALL: [DataType; 6] = [
        DataType::Bool,
        DataType::Int,
        DataType::Float,
        DataType::Vec2,
        DataType::Vec3,
        DataType::Color,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            DataType::Bool => "bool",
            DataType::Int => "int",
            DataType::Float => "float",
            DataType::Vec2 => "vec2",
            DataType::Vec3 => "vec3",
            DataType::Color => "color",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Values {
    Bool(Vec<bool>),
    Int(Vec<i32>),
    Float(Vec<f32>),
    Vec2(Vec<Vec2>),
    Vec3(Vec<Vec3>),
    Color(Vec<Vec4>),
}

macro_rules! each {
    ($values:expr, $list:ident => $body:expr) => {
        match $values {
            $crate::values::Values::Bool($list) => $body,
            $crate::values::Values::Int($list) => $body,
            $crate::values::Values::Float($list) => $body,
            $crate::values::Values::Vec2($list) => $body,
            $crate::values::Values::Vec3($list) => $body,
            $crate::values::Values::Color($list) => $body,
        }
    };
}

pub(crate) use each;

pub trait AttributeValue: Copy + PartialEq + std::fmt::Debug + Send + Sync + 'static {
    const DATA_TYPE: DataType;
    const DEFAULT: Self;

    fn slice(values: &Values) -> Option<&[Self]>;
    fn slice_mut(values: &mut Values) -> Option<&mut [Self]>;
    fn wrap(values: Vec<Self>) -> Values;
    fn mix(sources: impl Iterator<Item = (Self, f32)>) -> Self;
}

macro_rules! linear_value {
    ($ty:ty, $variant:ident, $default:expr, $zero:expr) => {
        impl AttributeValue for $ty {
            const DATA_TYPE: DataType = DataType::$variant;
            const DEFAULT: Self = $default;

            fn slice(values: &Values) -> Option<&[Self]> {
                match values {
                    Values::$variant(list) => Some(list),
                    _ => None,
                }
            }

            fn slice_mut(values: &mut Values) -> Option<&mut [Self]> {
                match values {
                    Values::$variant(list) => Some(list),
                    _ => None,
                }
            }

            fn wrap(values: Vec<Self>) -> Values {
                Values::$variant(values)
            }

            fn mix(sources: impl Iterator<Item = (Self, f32)>) -> Self {
                sources.fold($zero, |sum, (value, weight)| sum + value * weight)
            }
        }
    };
}

linear_value!(f32, Float, 0.0, 0.0);
linear_value!(Vec2, Vec2, Vec2::ZERO, Vec2::ZERO);
linear_value!(Vec3, Vec3, Vec3::ZERO, Vec3::ZERO);
linear_value!(Vec4, Color, Vec4::ONE, Vec4::ZERO);

impl AttributeValue for bool {
    const DATA_TYPE: DataType = DataType::Bool;
    const DEFAULT: Self = false;

    fn slice(values: &Values) -> Option<&[Self]> {
        match values {
            Values::Bool(list) => Some(list),
            _ => None,
        }
    }

    fn slice_mut(values: &mut Values) -> Option<&mut [Self]> {
        match values {
            Values::Bool(list) => Some(list),
            _ => None,
        }
    }

    fn wrap(values: Vec<Self>) -> Values {
        Values::Bool(values)
    }

    fn mix(mut sources: impl Iterator<Item = (Self, f32)>) -> Self {
        sources.any(|(value, weight)| value && weight > 0.0)
    }
}

impl AttributeValue for i32 {
    const DATA_TYPE: DataType = DataType::Int;
    const DEFAULT: Self = 0;

    fn slice(values: &Values) -> Option<&[Self]> {
        match values {
            Values::Int(list) => Some(list),
            _ => None,
        }
    }

    fn slice_mut(values: &mut Values) -> Option<&mut [Self]> {
        match values {
            Values::Int(list) => Some(list),
            _ => None,
        }
    }

    fn wrap(values: Vec<Self>) -> Values {
        Values::Int(values)
    }

    fn mix(sources: impl Iterator<Item = (Self, f32)>) -> Self {
        sources
            .map(|(value, weight)| value as f32 * weight)
            .sum::<f32>()
            .round() as i32
    }
}

fn default_of<T: AttributeValue>(_: &[T]) -> T {
    T::DEFAULT
}

fn mix_of<T: AttributeValue>(list: &[T], sources: &[(u32, f32)]) -> T {
    T::mix(
        sources
            .iter()
            .map(|&(index, weight)| (list[index as usize], weight)),
    )
}

impl Values {
    #[must_use]
    pub fn new(data_type: DataType, len: usize) -> Values {
        match data_type {
            DataType::Bool => Values::Bool(vec![bool::DEFAULT; len]),
            DataType::Int => Values::Int(vec![i32::DEFAULT; len]),
            DataType::Float => Values::Float(vec![f32::DEFAULT; len]),
            DataType::Vec2 => Values::Vec2(vec![Vec2::DEFAULT; len]),
            DataType::Vec3 => Values::Vec3(vec![Vec3::DEFAULT; len]),
            DataType::Color => Values::Color(vec![Vec4::DEFAULT; len]),
        }
    }

    #[must_use]
    pub fn data_type(&self) -> DataType {
        match self {
            Values::Bool(_) => DataType::Bool,
            Values::Int(_) => DataType::Int,
            Values::Float(_) => DataType::Float,
            Values::Vec2(_) => DataType::Vec2,
            Values::Vec3(_) => DataType::Vec3,
            Values::Color(_) => DataType::Color,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        each!(self, list => list.len())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn typed<T: AttributeValue>(&self) -> Option<&[T]> {
        T::slice(self)
    }

    #[must_use]
    pub fn typed_mut<T: AttributeValue>(&mut self) -> Option<&mut [T]> {
        T::slice_mut(self)
    }

    pub fn resize(&mut self, len: usize) {
        each!(self, list => {
            let fill = default_of(list);
            list.resize(len, fill);
        })
    }

    pub fn gather(&mut self, indices: &[u32]) {
        each!(self, list => {
            *list = parallel::gather(list, indices);
        })
    }

    pub fn copy(&mut self, from: usize, to: usize) {
        each!(self, list => list[to] = list[from])
    }

    pub fn interpolate(&mut self, sources: &[(u32, f32)], to: usize) {
        each!(self, list => list[to] = mix_of(list, sources))
    }

    pub fn push_interpolated(&mut self, sources: &[(u32, f32)]) {
        each!(self, list => {
            let mixed = mix_of(list, sources);
            list.push(mixed);
        })
    }

    pub fn push_copy(&mut self, from: usize) {
        each!(self, list => {
            let copied = list[from];
            list.push(copied);
        })
    }

    pub fn extend(&mut self, other: &Values) {
        let converted;
        let other = match other.data_type() == self.data_type() {
            true => other,
            false => {
                converted = other.convert(self.data_type());
                &converted
            }
        };
        match (self, other) {
            (Values::Bool(list), Values::Bool(added)) => list.extend_from_slice(added),
            (Values::Int(list), Values::Int(added)) => list.extend_from_slice(added),
            (Values::Float(list), Values::Float(added)) => list.extend_from_slice(added),
            (Values::Vec2(list), Values::Vec2(added)) => list.extend_from_slice(added),
            (Values::Vec3(list), Values::Vec3(added)) => list.extend_from_slice(added),
            (Values::Color(list), Values::Color(added)) => list.extend_from_slice(added),
            _ => unreachable!("converted values share the data type"),
        }
    }

    #[must_use]
    pub fn convert(&self, to: DataType) -> Values {
        if self.data_type() == to {
            return self.clone();
        }
        match to {
            DataType::Bool => Values::Bool(self.map(to_bool)),
            DataType::Int => Values::Int(self.map(to_int)),
            DataType::Float => Values::Float(self.map(to_float)),
            DataType::Vec2 => Values::Vec2(self.map(to_vec2)),
            DataType::Vec3 => Values::Vec3(self.map(to_vec3)),
            DataType::Color => Values::Color(self.map(to_color)),
        }
    }

    fn map<T>(&self, convert: impl Fn(Scalar) -> T) -> Vec<T> {
        match self {
            Values::Bool(list) => list
                .iter()
                .map(|&value| convert(Scalar::Bool(value)))
                .collect(),
            Values::Int(list) => list
                .iter()
                .map(|&value| convert(Scalar::Int(value)))
                .collect(),
            Values::Float(list) => list
                .iter()
                .map(|&value| convert(Scalar::Float(value)))
                .collect(),
            Values::Vec2(list) => list
                .iter()
                .map(|&value| convert(Scalar::Vec2(value)))
                .collect(),
            Values::Vec3(list) => list
                .iter()
                .map(|&value| convert(Scalar::Vec3(value)))
                .collect(),
            Values::Color(list) => list
                .iter()
                .map(|&value| convert(Scalar::Color(value)))
                .collect(),
        }
    }
}

#[derive(Clone, Copy)]
enum Scalar {
    Bool(bool),
    Int(i32),
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Color(Vec4),
}

fn luminance(color: Vec4) -> f32 {
    color.truncate().dot(LUMINANCE)
}

fn to_bool(value: Scalar) -> bool {
    match value {
        Scalar::Bool(value) => value,
        Scalar::Int(value) => value > 0,
        Scalar::Float(value) => value > 0.0,
        Scalar::Vec2(value) => value != Vec2::ZERO,
        Scalar::Vec3(value) => value != Vec3::ZERO,
        Scalar::Color(value) => luminance(value) > 0.0,
    }
}

fn to_float(value: Scalar) -> f32 {
    match value {
        Scalar::Bool(value) => f32::from(u8::from(value)),
        Scalar::Int(value) => value as f32,
        Scalar::Float(value) => value,
        Scalar::Vec2(value) => (value.x + value.y) / 2.0,
        Scalar::Vec3(value) => (value.x + value.y + value.z) / 3.0,
        Scalar::Color(value) => luminance(value),
    }
}

fn to_int(value: Scalar) -> i32 {
    match value {
        Scalar::Bool(value) => i32::from(value),
        Scalar::Int(value) => value,
        other => to_float(other) as i32,
    }
}

fn to_vec2(value: Scalar) -> Vec2 {
    match value {
        Scalar::Vec2(value) => value,
        Scalar::Vec3(value) => value.truncate(),
        Scalar::Color(value) => Vec2::new(value.x, value.y),
        other => Vec2::splat(to_float(other)),
    }
}

fn to_vec3(value: Scalar) -> Vec3 {
    match value {
        Scalar::Vec2(value) => value.extend(0.0),
        Scalar::Vec3(value) => value,
        Scalar::Color(value) => value.truncate(),
        other => Vec3::splat(to_float(other)),
    }
}

fn to_color(value: Scalar) -> Vec4 {
    match value {
        Scalar::Bool(value) => match value {
            true => Vec4::ONE,
            false => Vec4::new(0.0, 0.0, 0.0, 1.0),
        },
        Scalar::Vec2(value) => Vec4::new(value.x, value.y, 0.0, 1.0),
        Scalar::Vec3(value) => value.extend(1.0),
        Scalar::Color(value) => value,
        other => Vec3::splat(to_float(other)).extend(1.0),
    }
}

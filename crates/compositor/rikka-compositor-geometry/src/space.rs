#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Logical;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Physical;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Buffer;

pub(crate) fn space_name<S>() -> &'static str {
    let full = std::any::type_name::<S>();
    full.rsplit("::").next().unwrap_or(full)
}

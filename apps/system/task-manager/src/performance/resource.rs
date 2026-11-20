#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Resource {
    Cpu,
    Memory,
    Disk(String),
    Network(String),
    Gpu(String),
}

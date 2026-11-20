use rikka_hifi_core::DeviceId;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Output {
    #[default]
    Shared,
    Sink(String),
    Exclusive(DeviceId),
}

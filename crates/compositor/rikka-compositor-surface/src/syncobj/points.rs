use super::device::TimelinePoint;

#[derive(Debug, Clone, Default)]
pub(crate) struct SyncPoints {
    pub(crate) acquire: Option<TimelinePoint>,
    pub(crate) release: Option<TimelinePoint>,
}

use slotmap::new_key_type;

new_key_type! {
    pub struct SurfaceId;
    pub struct BufferId;
    pub struct BlockerId;
}

new_key_type! {
    pub(crate) struct PoolId;
    pub(crate) struct UseId;
    pub(crate) struct RegionId;
    pub(crate) struct ParamsId;
    pub(crate) struct TimelineId;
    pub(crate) struct CreatorId;
    pub(crate) struct DescriptionId;
}

rikka_wayland_wire::slot_keys!(
    SurfaceId,
    BufferId,
    PoolId,
    RegionId,
    ParamsId,
    TimelineId,
    CreatorId,
    DescriptionId,
);

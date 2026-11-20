use slotmap::new_key_type;

new_key_type! {
    pub(crate) struct SourceId;
    pub(crate) struct DragOfferId;
}

rikka_wayland_wire::slot_keys!(SourceId, DragOfferId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompositorSource(pub(crate) SourceId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForeignOffer(pub(crate) DragOfferId);

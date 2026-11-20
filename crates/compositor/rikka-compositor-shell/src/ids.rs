use slotmap::new_key_type;

new_key_type! {
    pub struct ToplevelId;
    pub struct PopupId;
    pub struct LayerId;
    pub struct LockId;
    pub struct LockSurfaceId;
    pub struct ForeignToplevelId;
}

new_key_type! {
    pub(crate) struct WmBaseId;
    pub(crate) struct PositionerId;
    pub(crate) struct ActivationDraftId;
    pub(crate) struct IconDraftId;
    pub(crate) struct ExportId;
    pub(crate) struct ImportId;
    pub(crate) struct ToplevelDragId;
}

rikka_wayland_wire::slot_keys!(
    ToplevelId,
    PopupId,
    LayerId,
    LockId,
    LockSurfaceId,
    ForeignToplevelId,
    WmBaseId,
    PositionerId,
    ActivationDraftId,
    IconDraftId,
    ExportId,
    ImportId,
    ToplevelDragId,
);
